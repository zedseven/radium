// Uses
use std::time::Duration;

use anyhow::Context;
use parse_duration::parse as parse_duration;
use poise::{
	command,
	serenity::model::{
		guild::Guild,
		id::{ChannelId, GuildId, UserId},
		misc::Mentionable,
	},
};
use rand::thread_rng;
use shuffle::{irs::Irs, shuffler::Shuffler};
use sponsor_block::Action;
use url::Url;

use crate::{
	constants::{
		MILLIS_PER_SECOND,
		MILLIS_PER_SECOND_F32,
		SPONSOR_BLOCK_ACCEPTED_ACTIONS,
		SPONSOR_BLOCK_ACCEPTED_CATEGORIES,
	},
	segments::SkipSegment,
	util::{
		create_linked_title,
		display_timecode,
		display_timecode_f32,
		push_chopped_str,
		reply,
		reply_embed,
		uri_is_url,
	},
	Error,
	PoiseContext,
};

// Constants
const MAX_DESCRIPTION_LENGTH: usize = 2048;
const DESCRIPTION_LENGTH_CUTOFF: usize = MAX_DESCRIPTION_LENGTH - 512;
const MAX_LIST_ENTRY_LENGTH: usize = 60;
const MAX_SINGLE_ENTRY_LENGTH: usize = 40;
const UNKNOWN_TITLE: &str = "Unknown title";
const LIVE_INDICATOR: &str = "\u{1f534} **LIVE**";

// Functions
async fn join_internal(ctx: PoiseContext<'_>, announce_success: bool) -> Result<Guild, ()> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.")
			.await
			.ok();
		return Err(());
	};

	let channel_id = if let Some(channel) = authour_channel_id(&guild, ctx.author().id) {
		channel
	} else {
		reply(ctx, "You must use this command while in a voice channel.")
			.await
			.ok();
		return Err(());
	};

	let (_, handler) = ctx.data().songbird.join_gateway(guild.id, channel_id).await;
	match handler {
		Ok(connection_info) => {
			if let Err(e) = ctx
				.data()
				.lavalink
				.create_session_with_songbird(&connection_info)
				.await
			{
				reply(
					ctx,
					format!("Error joining {}: {}", channel_id.mention(), e),
				)
				.await
				.ok();
				return Err(());
			}
		}
		Err(e) => {
			reply(
				ctx,
				format!("Error joining {}: {}", channel_id.mention(), e),
			)
			.await
			.ok();
			return Err(());
		}
	};

	if announce_success {
		reply(ctx, format!("Joined: {}", channel_id.mention()))
			.await
			.ok();
	}
	Ok(guild)
}

fn authour_channel_id(guild: &Guild, authour_id: UserId) -> Option<ChannelId> {
	guild
		.voice_states
		.get(&authour_id)
		.and_then(|voice_state| voice_state.channel_id)
}

/// Have Radium join the voice channel you're in.
#[command(prefix_command, slash_command, category = "Playback", aliases("j"))]
pub async fn join(ctx: PoiseContext<'_>) -> Result<(), Error> {
	join_internal(ctx, true).await.ok();

	Ok(())
}

/// Have Radium leave the voice channel it's in, if any.
#[command(prefix_command, slash_command, category = "Playback", aliases("l"))]
pub async fn leave(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let songbird = &ctx.data().songbird;

	if songbird.get(guild_id).is_some() {
		if let Err(e) = songbird.remove(guild_id).await {
			reply(ctx, format!("Error leaving voice channel: {}", e)).await?;
		}

		let lavalink = &ctx.data().lavalink;
		lavalink.destroy(guild_id.0).await?;

		reply(ctx, "Left the voice channel.").await?;
	} else {
		reply(ctx, "Not in a voice channel.").await?;
	}

	Ok(())
}

/// Queue up a song or playlist from YouTube, Twitch, Vimeo, SoundCloud, etc.
///
/// Spotify is only supported with [a Lavalink plugin](https://github.com/Topis-Lavalink-Plugins/Topis-Source-Managers-Plugin).
///
/// If Radium is provided with a URL, it will queue up all tracks it finds.
/// Otherwise it will search the query on YouTube and queue up the first result.
/// Age-restricted videos likely won't work.
///
/// You may also use this command with attachments (audio or video files),
/// though in that case you have to use the non-slash version of the command.
#[command(prefix_command, slash_command, category = "Playback", aliases("p"))]
pub async fn play(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What to play."]
	query: String,
) -> Result<(), Error> {
	play_internal(ctx, query.as_str(), false).await
}

/// Play a playlist, shuffled.
///
/// This is identical to the `play` command, except that it shuffles the tracks
/// before playing.
#[command(
	prefix_command,
	slash_command,
	category = "Playback",
	rename = "shuffleplay",
	aliases("sp", "ps", "playshuffled")
)]
pub async fn play_shuffled(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What to play."]
	query: String,
) -> Result<(), Error> {
	play_internal(ctx, query.as_str(), true).await
}

/// The internal implementation of `play` and `play_shuffled`.
async fn play_internal(ctx: PoiseContext<'_>, query: &str, shuffle: bool) -> Result<(), Error> {
	let guild = match join_internal(ctx, false).await {
		Ok(guild_result) => guild_result,
		Err(_) => return Ok(()),
	};

	let lavalink = &ctx.data().lavalink;

	let query_trimmed = query.trim();
	if query_trimmed.is_empty() {
		reply(ctx, "The query must not be empty.").await?;
		return Ok(());
	}

	let mut queueable_tracks = Vec::new();

	// Queue up any attachments
	if let PoiseContext::Prefix(prefix_ctx) = ctx {
		for attachment in &prefix_ctx.msg.attachments {
			// Verify the attachment is playable
			match &attachment.content_type {
				Some(t) => {
					if !t.starts_with("audio") && !t.starts_with("video") {
						continue;
					}
				}
				None => continue,
			}

			// Queue it up
			let mut query_result = lavalink.auto_search_tracks(&attachment.url).await?;
			for track in &mut query_result.tracks {
				track.info = match &track.info {
					Some(old_info) => {
						let mut new_info = old_info.clone();
						if old_info.title.eq(UNKNOWN_TITLE) {
							new_info.title = attachment.filename.clone();
						}
						Some(new_info)
					}
					None => None,
				}
			}
			queueable_tracks.extend_from_slice(&query_result.tracks);
		}
	}

	// Load the command query - if playable attachments were also with the message,
	// the attachments are queued first
	let query_information = lavalink.auto_search_tracks(query_trimmed).await?;

	let is_url = Url::parse(query_trimmed).is_ok();

	// If the query was a URL, then it's likely a playlist where all retrieved
	// tracks are desired - otherwise, only queue the top result
	let query_tracks = if is_url {
		query_information.tracks.len()
	} else {
		1
	};

	queueable_tracks.extend_from_slice(
		&query_information
			.tracks
			.iter()
			.take(query_tracks)
			.cloned()
			.collect::<Vec<_>>(),
	);

	let queueable_tracks_len = queueable_tracks.len();
	if queueable_tracks_len == 0 {
		reply(ctx, "Could not find anything for the search query.").await?;
		return Ok(());
	}

	// For URLs that point to raw files, Lavalink seems to just return them with a
	// title of "Unknown title" - this is a slightly hacky solution to set the title
	// to the filename of the raw file
	if is_url && query_tracks == 1 {
		let track_info = &mut queueable_tracks[queueable_tracks_len - 1];
		if track_info.info.is_some() && track_info.info.as_ref().unwrap().title.eq(UNKNOWN_TITLE) {
			track_info.info = match &track_info.info {
				Some(old_info) => {
					let mut new_info = old_info.clone();
					new_info.title = Url::parse(old_info.uri.as_str())
						.expect(
							"unable to parse track info URI when it should have been guaranteed \
							 to be valid",
						)
						.path_segments()
						.expect("unable to parse URI as a proper path")
						.last()
						.expect("unable to find the last path segment of URI")
						.to_owned();
					Some(new_info)
				}
				None => None,
			};
		}
	}

	// Shuffle if necessary
	if query_tracks > 1 && shuffle {
		let mut rng = thread_rng();
		let mut inverse_riffle_shuffler = Irs::default();
		inverse_riffle_shuffler
			.shuffle(&mut queueable_tracks, &mut rng)
			.ok(); // Ignore the error here because if the shuffle fails (which it never
		 // should) we want to continue
	}

	// Queue the tracks up
	let mut new_first_track_duration = None;
	for (index, track) in queueable_tracks.iter().enumerate() {
		let mut new_start_time = None;

		// YouTube SponsorBlock integration
		let track_identifier_opt = track.info.as_ref().map(|i| &i.identifier);
		let mut cache_track_with_none = true;
		'sponsorblock: {
			let track_identifier = if let Some(identifier) = track_identifier_opt {
				identifier
			} else {
				break 'sponsorblock;
			};

			// If we already have the segments for this video cached, we don't need to fetch
			// them again
			{
				let mut segment_data_handle = ctx.data().segment_data.lock().unwrap();
				if let Some(Some(segments)) =
					segment_data_handle.cached_segments.get(track_identifier)
				{
					// Load the special start and end times if necessary
					if !segments.is_empty() && segments[0].is_at_start {
						new_start_time = Some(Duration::from_secs_f32(segments[0].end));
					}
					// Break
					cache_track_with_none = false;
					break 'sponsorblock;
				}
			}

			if let Some(info) = &track.info {
				// Lavalink seems to round track length to the nearest second(?).
				const TRACK_ENDING_IMPRECISION: f32 = 1.0;
				// The maximum distance between two segments to combine.
				const SEGMENT_COMBINE_THRESHOLD: f32 = 0.35;
				// The minimum length a segment should be.
				const SEGMENT_LENGTH_THRESHOLD: f32 = 0.5;
				// The maximum difference from the submission video length to accept.
				const DURATION_DISCARD_THRESHOLD: f32 = 1.25;

				// No point if it's a stream
				if !info.is_seekable {
					break 'sponsorblock;
				}

				let parsed_uri = Url::parse(&info.uri).expect(
					"unable to parse track info URI when it should have been guaranteed to be \
					 valid",
				);

				if let Some(video_id) = get_youtube_video_id(&parsed_uri) {
					if let Ok(segments) = ctx
						.data()
						.sponsor_block
						.fetch_segments(
							&video_id,
							SPONSOR_BLOCK_ACCEPTED_CATEGORIES,
							SPONSOR_BLOCK_ACCEPTED_ACTIONS,
						)
						.await
					{
						// Calculate the track duration
						let track_duration = info.length as f32 / MILLIS_PER_SECOND_F32;
						// Get the pertinent information and filter out segments that may be
						// incorrect (submitted before some edit to the video length that
						// invalidates the timecodes)
						#[allow(clippy::wildcard_enum_match_arm)]
						let mut skip_timecodes = segments
							.iter()
							.filter(|s| {
								// Because some segments were added before video durations started
								// being recorded
								if let Some(video_duration_upon_submission) =
									s.video_duration_on_submission
								{
									(video_duration_upon_submission - track_duration).abs()
										<= DURATION_DISCARD_THRESHOLD
								} else {
									true
								}
							})
							.filter_map(|s| match &s.action {
								Action::Skip(start, end) | Action::Mute(start, end) => {
									Some(SkipSegment {
										start: *start,
										end: *end,
										is_at_start: false,
										is_at_end: false,
									})
								}
								_ => None,
							})
							.collect::<Vec<_>>();
						// Ensure the segments are ordered by their time in the content
						skip_timecodes
							.sort_unstable_by_key(|t| (t.start * MILLIS_PER_SECOND_F32) as u32);
						// Combine segments that are close together
						let mut skip_timecodes_len = skip_timecodes.len();
						if skip_timecodes_len > 1 {
							for i in (1..skip_timecodes_len).rev() {
								if skip_timecodes[i].start - skip_timecodes[i - 1].end
									> SEGMENT_COMBINE_THRESHOLD
								{
									continue;
								}
								skip_timecodes[i - 1].end = skip_timecodes[i].end;
								skip_timecodes.remove(i);
							}
						}
						// Remove segments that are too short to be worth skipping with the Lavalink
						// seek delay
						skip_timecodes = skip_timecodes
							.drain(..)
							.filter(|t| t.end - t.start >= SEGMENT_LENGTH_THRESHOLD)
							.collect::<Vec<_>>();

						// Final processing
						skip_timecodes_len = skip_timecodes.len();
						if skip_timecodes_len > 0 {
							// Store the new duration, without the skipped segments, for the first
							// track
							if index == 0 {
								let new_track_duration = info.length
									- (skip_timecodes.iter().map(|t| t.end - t.start).sum::<f32>()
										* MILLIS_PER_SECOND_F32) as u64;
								// The track durations are displayed with 1s precision, so there's
								// no point in setting the new track duration if it's a difference
								// of <1s
								if new_track_duration <= info.length - MILLIS_PER_SECOND {
									new_first_track_duration = Some(new_track_duration);
								}
							}

							// Set the start time for the track if there's a segment right at the
							// beginning
							if skip_timecodes[0].start < TRACK_ENDING_IMPRECISION {
								skip_timecodes[0].is_at_start = true;
								new_start_time =
									Some(Duration::from_secs_f32(skip_timecodes[0].end));
							}
							// Set the end segment's is_at_end value if it's at the very end
							if (track_duration - skip_timecodes[skip_timecodes_len - 1].end).abs()
								< TRACK_ENDING_IMPRECISION
							{
								skip_timecodes[skip_timecodes_len - 1].is_at_end = true;
							}
						}

						// Cache the segments if there's segments to cache
						if skip_timecodes.is_empty() {
							break 'sponsorblock;
						}
						{
							let mut segment_data_handle = ctx.data().segment_data.lock().unwrap();
							segment_data_handle
								.cached_segments
								.put(track_identifier.clone(), Some(skip_timecodes));
						}
						cache_track_with_none = false;
					}
				}
			}
		}
		// If no segments were found, cache that fact so we don't have to check the next
		// time the video is requested
		if cache_track_with_none {
			if let Some(track_identifier) = track_identifier_opt {
				let mut segment_data_handle = ctx.data().segment_data.lock().unwrap();
				segment_data_handle
					.cached_segments
					.put(track_identifier.clone(), None);
			}
		}

		// Queue
		let mut queueable = lavalink.play(guild.id.0, track.clone());
		queueable.requester(ctx.author().id.0);
		if let Some(start_time) = new_start_time {
			queueable.start_time(start_time);
		}
		if let Err(e) = queueable.queue().await {
			reply(ctx, "Failed to queue up query result.").await?;
			eprintln!("Failed to queue up query result: {}", e);
			return Ok(());
		};
	}

	// Update the queued count for the guild
	{
		let mut hash_map = ctx.data().queued_count.lock().unwrap();
		let queued_count = hash_map.entry(guild.id).or_default();
		*queued_count += queueable_tracks_len;
	}

	// Notify the user of the added tracks
	if queueable_tracks_len == 1 {
		let track_info = queueable_tracks[0].info.as_ref().unwrap();
		reply(
			ctx,
			format!(
				"Added to queue: {} [{}]",
				create_linked_title(
					track_info.title.as_str(),
					track_info.uri.as_str(),
					MAX_SINGLE_ENTRY_LENGTH,
				),
				if track_info.is_stream {
					LIVE_INDICATOR.to_owned()
				} else if let Some(new_track_duration) = new_first_track_duration {
					format!(
						"{} ({})",
						display_timecode(track_info.length),
						display_timecode(new_track_duration)
					)
				} else {
					display_timecode(track_info.length)
				}
			),
		)
		.await?;
	} else {
		let mut desc = String::from("Requested by ");
		desc.push_str(ctx.author().mention().to_string().as_str());
		desc.push('\n');
		for (i, track) in queueable_tracks.iter().enumerate() {
			let track_info = track.info.as_ref().unwrap();
			desc.push_str("- [");
			push_chopped_str(&mut desc, track_info.title.as_str(), MAX_LIST_ENTRY_LENGTH);
			desc.push_str("](");
			desc.push_str(track_info.uri.as_str());
			desc.push(')');
			if i < queueable_tracks_len - 1 {
				desc.push('\n');
				if desc.len() > DESCRIPTION_LENGTH_CUTOFF {
					desc.push_str("*\u{2026}the rest has been clipped*");
					break;
				}
			}
		}
		reply_embed(ctx, |e| {
			e.title(format!("Added {} Tracks:", queueable_tracks_len))
				.description(desc)
		})
		.await?;
	}

	Ok(())
}
/// Parses out the YouTube video ID from a video URL.
fn get_youtube_video_id(uri: &Url) -> Option<String> {
	if let Some(host) = uri.host_str() {
		if host.ends_with("youtube.com") {
			if let Some(query) = uri.query() {
				let query_parameters = query.split('&');
				for parameter in query_parameters {
					if let Some(stripped) = parameter.strip_prefix("v=") {
						return Some(stripped.to_owned());
					}
				}
				None
			} else {
				None
			}
		} else if host.ends_with("youtu.be") {
			Some(
				uri.path_segments()
					.expect("unable to parse URI as a proper path")
					.last()
					.expect("unable to find the last path segment of URI")
					.to_owned(),
			)
		} else {
			None
		}
	} else {
		None
	}
}

/// Text-to-speech in the current voice channel.
///
/// This command relies on functionality added by [a Lavalink plugin](https://github.com/DuncteBot/skybot-lavalink-plugin),
/// and will not work without it.
#[command(
	prefix_command,
	slash_command,
	category = "Playback",
	aliases("texttospeech", "say", "speak")
)]
#[cfg(feature = "tts")]
pub async fn tts(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What to say."]
	message: String,
) -> Result<(), Error> {
	// Constants
	/// The limit in Lavalink seems to be 200 - this is to include a buffer
	const MAX_SINGLE_FRAGMENT_SIZE: usize = 185;

	async fn queue_fragment(
		ctx: PoiseContext<'_>,
		guild_id: GuildId,
		fragment: &str,
	) -> Result<(), Error> {
		let lavalink = &ctx.data().lavalink;

		// Fetch the fragment
		let tts_result = lavalink.get_tracks(format!("speak:{}", fragment)).await?;
		if tts_result.tracks.is_empty() {
			reply(ctx, "TTS failed.").await?;
			return Ok(());
		}

		// Queue it
		let mut queueable = lavalink.play(guild_id, tts_result.tracks[0].clone());
		queueable.requester(ctx.author().id.0);
		if let Err(e) = queueable.queue().await {
			reply(ctx, "Failed to queue up query result.").await?;
			eprintln!("Failed to queue up query result: {}", e);
			return Ok(());
		};

		Ok(())
	}

	// Sanitisation
	let message_trimmed = message.trim();
	if message_trimmed.is_empty() {
		reply(ctx, "Your message cannot be empty.").await?;
		return Ok(());
	}
	/*if message_trimmed.len() > MAX_SINGLE_FRAGMENT_SIZE {
		reply(
			ctx,
			format!(
				"Your message must be less than {} characters long.",
				MAX_SINGLE_FRAGMENT_SIZE
			),
		)
		.await?;
		return Ok(());
	}*/

	// Join the channel and set up
	let guild = match join_internal(ctx, false).await {
		Ok(guild_result) => guild_result,
		Err(_) => return Ok(()),
	};

	// Queue it up
	let mut queued_tracks = 0;
	let mut message_buffer = String::with_capacity(MAX_SINGLE_FRAGMENT_SIZE);
	for word in message_trimmed.split_whitespace() {
		// Catch the edge case of a long string with no whitespace
		if word.len() > MAX_SINGLE_FRAGMENT_SIZE {
			reply(
				ctx,
				format!(
					"A single word cannot be longer than {} characters.",
					MAX_SINGLE_FRAGMENT_SIZE
				),
			)
			.await?;
			return Ok(());
		}

		// If the buffer is full, queue it up and flush it
		if message_buffer.len() + word.len() > MAX_SINGLE_FRAGMENT_SIZE {
			queue_fragment(ctx, guild.id, message_buffer.trim_end()).await?;
			message_buffer.clear();
			queued_tracks += 1;
		}

		// Push the next word onto the buffer
		message_buffer.push_str(word);
		message_buffer.push(' '); // Technically this isn't accounted for above, but we
		                  // have a buffer between our fragment limit and the
		                  // actual limit anyways
	}
	// Queue up whatever is remaining
	if !message_buffer.is_empty() {
		queue_fragment(ctx, guild.id, message_buffer.trim_end()).await?;
		queued_tracks += 1;
	}

	// Update the queued count for the guild
	{
		let mut hash_map = ctx.data().queued_count.lock().unwrap();
		let queued_count = hash_map.entry(guild.id).or_default();
		*queued_count += queued_tracks;
	}

	// Response
	let mut response = String::from("Added to queue: \"");
	push_chopped_str(&mut response, message_trimmed, MAX_SINGLE_FRAGMENT_SIZE);
	response.push('"');
	reply(ctx, response).await?;

	Ok(())
}

/// Skip the current track.
#[command(
	prefix_command,
	slash_command,
	category = "Playback",
	aliases("next", "stop", "n", "s")
)]
pub async fn skip(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	if let Some(track) = lavalink.skip(guild_id.0).await {
		let track_info = track.track.info.as_ref().unwrap();
		// If the queue is now empty, the player needs to be stopped
		if lavalink
			.nodes()
			.await
			.get(&guild_id.0)
			.unwrap()
			.queue
			.is_empty()
		{
			lavalink
				.stop(guild_id.0)
				.await
				.with_context(|| "failed to stop playback of the current track".to_owned())?;
		}
		reply(
			ctx,
			format!(
				"Skipped: {}",
				create_linked_title(
					track_info.title.as_str(),
					track_info.uri.as_str(),
					MAX_SINGLE_ENTRY_LENGTH,
				)
			),
		)
		.await?;
	} else {
		reply(ctx, "Nothing to skip.").await?;
	}

	Ok(())
}

/// Pause the current track.
///
/// The opposite of `resume`.
#[command(prefix_command, slash_command, category = "Playback")]
pub async fn pause(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	if let Err(e) = lavalink.pause(guild_id.0).await {
		reply(ctx, "Failed to pause playback.").await?;
		eprintln!("Failed to pause playback: {}", e);
		return Ok(());
	};

	reply(ctx, "Paused playback.").await?;

	Ok(())
}

/// Resume the current track.
///
/// The opposite of `pause`.
#[command(prefix_command, slash_command, category = "Playback")]
pub async fn resume(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	if let Err(e) = lavalink.resume(guild_id.0).await {
		reply(ctx, "Failed to resume playback.").await?;
		eprintln!("Failed to resume playback: {}", e);
		return Ok(());
	};

	reply(ctx, "Resumed playback.").await?;

	Ok(())
}

/// Seek to a specific time in the current track.
///
/// You can specify the time to skip to as a timecode (`2:35`) or as individual
/// time values (`2m35s`).
///
/// If the time specified is past the end of the track, the track ends.
#[command(
	prefix_command,
	slash_command,
	category = "Playback",
	aliases("scrub", "jump")
)]
pub async fn seek(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What time to skip to."]
	time: String,
) -> Result<(), Error> {
	// Constants
	const COLON: char = ':';
	const DECIMAL: char = '.';

	// Parse the time - this is a little hacky and gross, but it allows for support
	// of timecodes like `2:35`. This is more ergonomic for users than something
	// like `2m35s`, and this way both formats are supported.
	let mut invalid_value = false;
	let mut time_prepared = String::with_capacity(time.len());
	'prepare_time: for timecode in time.split_whitespace() {
		// First iteration to find indices and make sure the timecode is valid
		let mut colon_index_first = None;
		let mut colon_index_second = None;
		let mut decimal_index = None;
		for (i, c) in timecode.chars().enumerate() {
			if c == COLON {
				if colon_index_first.is_none() {
					colon_index_first = Some(i);
				} else if colon_index_second.is_none() {
					colon_index_second = Some(i);
				} else {
					// Maximum of two colons in a timecode
					invalid_value = true;
					break 'prepare_time;
				}
				if decimal_index.is_some() {
					// Colons don't come after decimals
					invalid_value = true;
					break 'prepare_time;
				}
			} else if c == DECIMAL {
				if decimal_index.is_none() {
					decimal_index = Some(i);
				} else {
					// Only one decimal value
					invalid_value = true;
					break 'prepare_time;
				}
			}
		}

		// Second iteration using those indices to convert the timecode to a duration
		// representation
		let mut new_word = String::with_capacity(timecode.len());
		for (i, c) in timecode.chars().enumerate() {
			if colon_index_first.is_some() && i == colon_index_first.unwrap() {
				if colon_index_second.is_some() {
					new_word.push('h');
				} else {
					new_word.push('m');
				}
			} else if colon_index_second.is_some() && i == colon_index_second.unwrap() {
				new_word.push('m');
			} else if decimal_index.is_some() && i == decimal_index.unwrap() {
				new_word.push('s');
			} else {
				new_word.push(c);
			}
		}
		if decimal_index.is_some() {
			new_word.push_str("ms");
		} else if colon_index_first.is_some() {
			new_word.push('s');
		}

		// Push the prepared timecode to the result
		time_prepared.push_str(new_word.as_str());
		time_prepared.push(' ');
	}
	if invalid_value {
		reply(ctx, "Invalid value for time.").await?;
		return Ok(());
	}

	let time_dur = if let Ok(duration) = parse_duration(time_prepared.as_str()) {
		duration
	} else {
		reply(ctx, "Invalid value for time.").await?;
		return Ok(());
	};

	// Seek to the parsed time
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	if let Err(e) = lavalink.seek(guild_id.0, time_dur).await {
		reply(ctx, "Failed to seek to the specified time.").await?;
		eprintln!("Failed to seek to the specified time: {}", e);
		return Ok(());
	};

	reply(ctx, "Scrubbed to the specified time.").await?;

	Ok(())
}

/// Clear the playback queue.
///
/// In addition to clearing the queue, this also resets the queue position for
/// new tracks. This is the only way this happens other than when the bot goes
/// offline.
#[command(prefix_command, slash_command, category = "Playback", aliases("c"))]
pub async fn clear(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	while lavalink.skip(guild_id.0).await.is_some() {}
	lavalink
		.stop(guild_id.0)
		.await
		.with_context(|| "failed to stop playback of the current track".to_owned())?;
	reply(ctx, "The queue is now empty.").await?;

	{
		let mut hash_map = ctx.data().queued_count.lock().unwrap();
		let queued_count = hash_map.entry(guild_id).or_default();
		*queued_count = 0;
	}

	Ok(())
}

/// Show what's currently playing, and how far along in the track Radium is.
///
/// If the track has a defined end point, a progress bar will be displayed.
/// Otherwise, if the track is a live stream, only the time it's been playing
/// will be displayed.
#[command(
	prefix_command,
	slash_command,
	category = "Playback",
	rename = "nowplaying",
	aliases("np", "position", "current", "rn")
)]
pub async fn now_playing(ctx: PoiseContext<'_>) -> Result<(), Error> {
	fn create_progress_display(length: Option<u64>, position: u64) -> String {
		const EMPTY_BLOCK: char = '\u{25b1}';
		const FULL_BLOCK: char = '\u{25b0}';
		const PROGRESS_BAR_SIZE: u64 = 10;

		let mut ret = String::new();
		if let Some(length_actual) = length {
			ret.push_str(display_timecode(position).as_str());
			ret.push(' ');
			let fill_point = position * PROGRESS_BAR_SIZE / length_actual;
			for _ in 0..fill_point {
				ret.push(FULL_BLOCK);
			}
			for _ in fill_point..PROGRESS_BAR_SIZE {
				ret.push(EMPTY_BLOCK);
			}
			ret.push(' ');
			ret.push_str(display_timecode(length_actual).as_str());
		} else {
			ret.push_str(display_timecode(position).as_str());
			ret.push(' ');
			ret.push_str(LIVE_INDICATOR);
		}
		ret
	}
	fn display_segments(segments: &[SkipSegment], length: u64) -> String {
		let mut ret = String::new();
		for segment in segments {
			// The is_at_start and is_at_end checks are so that there's a unified display,
			// since floating-point imprecision and track length rounding seem to often lead
			// to the segment time not exactly matching the actual value when displayed
			ret.push_str(
				format!(
					"- {} - {}",
					if segment.is_at_start {
						display_timecode(0)
					} else {
						display_timecode_f32(segment.start)
					},
					if segment.is_at_end {
						display_timecode(length)
					} else {
						display_timecode_f32(segment.end)
					}
				)
				.as_str(),
			);
			ret.push('\n');
		}
		ret
	}

	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	let now_playing_opt = if let Some(node) = lavalink.nodes().await.get(&guild_id.0) {
		node.now_playing.clone()
	} else {
		None
	};
	match now_playing_opt {
		Some(now_playing) => {
			let track_info = now_playing.track.info.as_ref().unwrap();
			let track_has_url_source = uri_is_url(track_info.uri.as_str());
			let track_segments = {
				let mut segment_data_handle = ctx.data().segment_data.lock().unwrap();
				segment_data_handle
					.cached_segments
					.get(&track_info.identifier)
					.cloned()
			};
			reply_embed(ctx, |e| {
				e.title("Now Playing")
					.field(
						"Track:",
						create_linked_title(
							track_info.title.as_str(),
							track_info.uri.as_str(),
							MAX_SINGLE_ENTRY_LENGTH,
						),
						false,
					)
					.field(
						"Requested By:",
						UserId(
							now_playing
								.requester
								.expect("expected a requester associated with a playing track")
								.0,
						)
						.mention(),
						false,
					);
				if track_has_url_source {
					e.field(
						"Progress:",
						create_progress_display(
							if track_info.is_stream {
								None
							} else {
								Some(track_info.length)
							},
							track_info.position,
						),
						false,
					);
				}
				if let Some(Some(segments)) = track_segments {
					e.field(
						"Skip Segments:",
						display_segments(&segments, track_info.length),
						false,
					);
				}
				e
			})
			.await?;
		}
		None => {
			reply(ctx, "Nothing is playing at the moment.").await?;
		}
	}

	Ok(())
}

/// Show the playback queue.
#[command(prefix_command, slash_command, category = "Playback", aliases("q"))]
pub async fn queue(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild_id = if let Some(guild_id) = ctx.guild_id() {
		guild_id
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lavalink = &ctx.data().lavalink;

	let mut something_in_queue = false;
	if let Some(node) = lavalink.nodes().await.get(&guild_id.0) {
		let queue = &node.queue;
		let queue_len = queue.len();

		if queue_len > 0 {
			something_in_queue = true;

			let global_queued_count = {
				let mut hash_map = ctx.data().queued_count.lock().unwrap();
				*hash_map.entry(guild_id).or_default()
			};
			let entry_offset = global_queued_count - queue_len;
			let number_width = global_queued_count.log10() as usize + 1;

			let mut desc = String::new();
			for (i, queued_track) in queue.iter().enumerate() {
				let track_info = queued_track.track.info.as_ref().unwrap();
				desc.push_str(format!("`{:01$}.` ", entry_offset + i + 1, number_width).as_str());
				desc.push_str(
					create_linked_title(
						track_info.title.as_str(),
						track_info.uri.as_str(),
						MAX_SINGLE_ENTRY_LENGTH,
					)
					.as_str(),
				);
				if i < queue_len - 1 {
					desc.push('\n');
					if desc.len() > DESCRIPTION_LENGTH_CUTOFF {
						desc.push_str("*\u{2026}the rest has been clipped*");
						break;
					}
				}
			}
			reply_embed(ctx, |e| {
				e.title(if queue_len == 1 {
					format!("Queue ({} total track):", queue_len)
				} else {
					format!("Queue ({} total tracks):", queue_len)
				})
				.description(desc)
			})
			.await?;
		}
	}
	if !something_in_queue {
		reply(ctx, "Nothing is in the queue.").await?;
	}

	Ok(())
}
