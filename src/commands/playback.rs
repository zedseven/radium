use std::time::Duration;

// Uses
use anyhow::Context;
use lavalink_rs::LavalinkClient;
use parse_duration::parse as parse_duration;
use poise::{
	command,
	serenity::model::{
		guild::Guild,
		id::{ChannelId, UserId},
		misc::Mentionable,
	},
};
use songbird::{
	id::{ChannelId as SongbirdChannelId, GuildId},
	Songbird,
};
use sponsor_block::ActionableSegment;
use url::Url;

use crate::{
	constants::ACCEPTED_CATEGORIES,
	segments::{SkipSegment, TRACK_IDENTIFIER_LENGTH},
	util::{chop_str, display_time_span, push_chopped_str, reply, reply_embed},
	Error,
	PoiseContext,
};

// Constants
const MAX_DESCRIPTION_LENGTH: usize = 2048;
const DESCRIPTION_LENGTH_CUTOFF: usize = MAX_DESCRIPTION_LENGTH - 512;
const MAX_LIST_ENTRY_LENGTH: usize = 60;
const MAX_SINGLE_ENTRY_LENGTH: usize = 40;
const UNKNOWN_TITLE: &str = "Unknown title";
const LIVE_INDICATOR: &str = "🔴 **LIVE**";

// Functions
async fn join_internal<G, C>(
	songbird: &Songbird,
	lavalink: &LavalinkClient,
	guild_id: G,
	channel_id: C,
) -> Result<(), Error>
where
	G: Into<GuildId>,
	C: Into<SongbirdChannelId>,
{
	let (_, handler) = songbird.join_gateway(guild_id, channel_id).await;

	match handler {
		Ok(connection_info) => Ok(lavalink
			.create_session(&connection_info)
			.await
			.map_err(Box::new)?),
		Err(e) => Err(Box::new(e)),
	}
}

fn authour_channel_id(guild: &Guild, authour_id: UserId) -> Option<ChannelId> {
	guild
		.voice_states
		.get(&authour_id)
		.and_then(|voice_state| voice_state.channel_id)
}

/// Have Radium join the voice channel you're in.
#[command(slash_command, aliases("j"))]
pub async fn join(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let channel_id = if let Some(channel) = authour_channel_id(&guild, ctx.author().id) {
		channel
	} else {
		reply(ctx, "You must use this command while in a voice channel.").await?;
		return Ok(());
	};

	match join_internal(
		&ctx.data().songbird,
		&ctx.data().lavalink,
		guild.id,
		channel_id,
	)
	.await
	{
		Ok(_) => reply(ctx, format!("Joined: {}", channel_id.mention())).await?,
		Err(e) => {
			reply(
				ctx,
				format!("Error joining {}: {}", channel_id.mention(), e),
			)
			.await?;
		}
	}

	Ok(())
}

/// Have Radium leave the voice channel it's in, if any.
#[command(slash_command, aliases("l"))]
pub async fn leave(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let manager = &ctx.data().songbird;

	if manager.get(guild.id).is_some() {
		if let Err(e) = manager.remove(guild.id).await {
			reply(ctx, format!("Error leaving voice channel: {}", e)).await?;
		}

		let lava_client = &ctx.data().lavalink;
		lava_client.destroy(guild.id.0).await?;

		reply(ctx, "Left the voice channel.").await?;
	} else {
		reply(ctx, "Not in a voice channel.").await?;
	}

	Ok(())
}

/// Queue up a song or playlist from YouTube, Twitch, Vimeo, SoundCloud, etc.
/// Spotify is sadly not supported.
///
/// If Radium is provided with a URL, it will queue up all tracks it finds.
/// Otherwise it will search the query on YouTube and queue up the first result.
/// Age-restricted videos likely won't work.
///
/// You may also use this command with attachments (audio or video files),
/// though in that case you have to use the non-slash version of the command.
#[command(slash_command, aliases("p"))]
pub async fn play(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What to play."]
	query: String,
) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let manager = &ctx.data().songbird;
	let lava_client = &ctx.data().lavalink;

	if manager.get(guild.id).is_none() {
		let channel_id = if let Some(channel) = authour_channel_id(&guild, ctx.author().id) {
			channel
		} else {
			reply(
				ctx,
				"You must use this command while either you or Radium is in a voice channel.",
			)
			.await?;
			return Ok(());
		};

		if let Err(e) = join_internal(manager, lava_client, guild.id, channel_id).await {
			reply(
				ctx,
				format!("Error joining {}: {}", channel_id.mention(), e),
			)
			.await?;
			return Ok(());
		}
	}

	let mut queueable_tracks = Vec::new();

	// Queue up any attachments
	if let PoiseContext::Prefix(prefix_ctx) = ctx {
		for attachment in &prefix_ctx.msg.attachments {
			// Verify the attachment is playable
			let playable_content = match &attachment.content_type {
				Some(t) => t.starts_with("audio") || t.starts_with("video"),
				None => false,
			};
			if !playable_content {
				continue;
			}

			// Queue it up
			let mut query_result = lava_client.auto_search_tracks(&attachment.url).await?;
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
	let query_information = lava_client.auto_search_tracks(&query).await?;

	let is_url = Url::parse(query.trim()).is_ok();

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
							"Unable to parse track info URI when it should have been guaranteed \
							 to be valid",
						)
						.path_segments()
						.expect("Unable to parse URI as a proper path")
						.last()
						.expect("Unable to find the last path segment of URI")
						.to_owned();
					Some(new_info)
				}
				None => None,
			};
		}
	}

	// Queue the tracks up
	for track in &queueable_tracks {
		let mut new_track_duration = None;
		let mut new_start_time = None;
		let mut new_end_time = None;

		// YouTube SponsorBlock integration
		'sponsorblock: {
			if let Some(info) = &track.info {
				const SEGMENT_COMBINE_THRESHOLD: f32 = 0.2;
				const SEGMENT_LENGTH_THRESHOLD: f32 = 0.5;
				const DURATION_DISCARD_THRESHOLD: f32 = 1.0;
				const MILLIS_PER_SECOND: f32 = 1000.0;

				// No point if it's a stream
				if !info.is_seekable {
					break 'sponsorblock;
				}

				let parsed_uri = Url::parse(&info.uri).expect(
					"Unable to parse track info URI when it should have been guaranteed to be \
					 valid",
				);

				if let Some(video_id) = get_youtube_video_id(&parsed_uri) {
					if let Ok(segments) = ctx
						.data()
						.sponsor_block
						.fetch_segments(&video_id, ACCEPTED_CATEGORIES)
						.await
					{
						dbg!(&segments);

						// Calculate the track duration
						let track_duration = info.length as f32 / MILLIS_PER_SECOND;

						// Get the pertinent information and filter out segments that may be
						// incorrect (submitted before some edit to the video length that
						// invalidates the timecodes)
						let mut skip_timecodes = segments
							.iter()
							.filter(|s| {
								(s.video_duration_on_submission - track_duration).abs()
									<= DURATION_DISCARD_THRESHOLD
							})
							.filter_map(|s| match &s.segment {
								ActionableSegment::Sponsor(t)
								| ActionableSegment::UnpaidSelfPromotion(t)
								| ActionableSegment::InteractionReminder(t)
								| ActionableSegment::IntermissionIntroAnimation(t)
								| ActionableSegment::EndcardsCredits(t)
								| ActionableSegment::NonMusic(t) => Some(SkipSegment {
									start: t.start,
									end: t.end,
									is_at_an_end: false,
								}),
								_ => None,
							})
							.collect::<Vec<_>>();
						// Ensure the segments are ordered by their time in the content
						skip_timecodes
							.sort_unstable_by_key(|t| (t.start * MILLIS_PER_SECOND) as u32);
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

						// Store the new duration, without the skipped segments
						new_track_duration = Some(
							info.length
								- (skip_timecodes.iter().map(|t| t.end - t.start).sum::<f32>()
									* MILLIS_PER_SECOND) as u64,
						);

						// Start & end times
						skip_timecodes_len = skip_timecodes.len();
						if skip_timecodes_len > 0 {
							// Set the start time for the track if there's a segment right at the
							// beginning
							if skip_timecodes[0].start <= SEGMENT_COMBINE_THRESHOLD {
								skip_timecodes[0].is_at_an_end = true;
								new_start_time =
									Some(Duration::from_secs_f32(skip_timecodes[0].end));
							}
							// Set the end time for the track if there's a segment right at the end
							if (info.length as f32 / MILLIS_PER_SECOND)
								- skip_timecodes[skip_timecodes_len - 1].end
								<= SEGMENT_COMBINE_THRESHOLD
							{
								skip_timecodes[skip_timecodes_len - 1].is_at_an_end = true;
								new_end_time = Some(Duration::from_secs_f32(
									skip_timecodes[skip_timecodes_len - 1].start,
								));
							}
						}

						dbg!(&skip_timecodes);

						// Cache the segments
						{
							let mut segment_data_handle = ctx.data().segment_data.lock().unwrap();
							segment_data_handle.cached_segments.insert(
								track.track[..TRACK_IDENTIFIER_LENGTH].to_owned(),
								skip_timecodes,
							);
							dbg!(&segment_data_handle);
						}
					}
				}
			}
		}

		// Queue
		let mut queueable = lava_client
			.play(guild.id.0, track.clone())
			.requester(ctx.author().id.0);
		if let Some(start_time) = new_start_time {
			queueable = queueable.start_time(start_time);
		}
		if let Some(end_time) = new_end_time {
			queueable = queueable.finish_time(end_time);
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
				"Added to queue: [{}]({}) [{}]",
				chop_str(track_info.title.as_str(), MAX_SINGLE_ENTRY_LENGTH),
				track_info.uri,
				if track_info.is_stream {
					LIVE_INDICATOR.to_owned()
				} else {
					display_time_span(track_info.length)
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
					desc.push_str("*…the rest has been clipped*");
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
					.expect("Unable to parse URI as a proper path")
					.last()
					.expect("Unable to find the last path segment of URI")
					.to_owned(),
			)
		} else {
			None
		}
	} else {
		None
	}
}

/// Skip the current track.
#[command(slash_command, aliases("next", "stop", "n", "s"))]
pub async fn skip(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	if let Some(track) = lava_client.skip(guild.id.0).await {
		let track_info = track.track.info.as_ref().unwrap();
		// If the queue is now empty, the player needs to be stopped
		if lava_client
			.nodes()
			.await
			.get(&guild.id.0)
			.unwrap()
			.queue
			.is_empty()
		{
			lava_client
				.stop(guild.id.0)
				.await
				.with_context(|| "Failed to stop playback of the current track".to_owned())?;
		}
		reply(
			ctx,
			format!(
				"Skipped: [{}]({})",
				chop_str(track_info.title.as_str(), MAX_SINGLE_ENTRY_LENGTH),
				track_info.uri
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
#[command(slash_command)]
pub async fn pause(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	if let Err(e) = lava_client.pause(guild.id.0).await {
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
#[command(slash_command)]
pub async fn resume(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	if let Err(e) = lava_client.resume(guild.id.0).await {
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
#[command(slash_command, aliases("scrub", "jump"))]
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
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	if let Err(e) = lava_client.seek(guild.id.0, time_dur).await {
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
#[command(slash_command, aliases("c"))]
pub async fn clear(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	while lava_client.skip(guild.id.0).await.is_some() {}
	lava_client
		.stop(guild.id.0)
		.await
		.with_context(|| "Failed to stop playback of the current track".to_owned())?;
	reply(ctx, "The queue is now empty.").await?;

	{
		let mut hash_map = ctx.data().queued_count.lock().unwrap();
		let queued_count = hash_map.entry(guild.id).or_default();
		*queued_count = 0;
	}

	Ok(())
}

/// Show what's currently playing, and how far in you are in the track.
///
/// If the track has a defined end point, a progress bar will be displayed.
/// Otherwise, if the track is a live stream, only the time it's been playing
/// will be displayed.
#[command(
	slash_command,
	rename = "nowplaying",
	aliases("np", "position", "current", "rn")
)]
pub async fn now_playing(ctx: PoiseContext<'_>) -> Result<(), Error> {
	fn create_progress_display(length: Option<u64>, position: u64) -> String {
		const EMPTY_BLOCK: char = '▱';
		const FULL_BLOCK: char = '▰';
		const PROGRESS_BAR_SIZE: u64 = 10;

		let mut ret = String::new();
		if let Some(length_actual) = length {
			ret.push_str(display_time_span(position).as_str());
			ret.push(' ');
			let fill_point = position * PROGRESS_BAR_SIZE / length_actual;
			for _ in 0..fill_point {
				ret.push(FULL_BLOCK);
			}
			for _ in fill_point..PROGRESS_BAR_SIZE {
				ret.push(EMPTY_BLOCK);
			}
			ret.push(' ');
			ret.push_str(display_time_span(length_actual).as_str());
		} else {
			ret.push_str(display_time_span(position).as_str());
			ret.push(' ');
			ret.push_str(LIVE_INDICATOR);
		}
		ret
	}

	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	let mut something_playing = false;
	if let Some(node) = lava_client.nodes().await.get(&guild.id.0) {
		if let Some(now_playing) = &node.now_playing {
			let track_info = now_playing.track.info.as_ref().unwrap();
			reply_embed(ctx, |e| {
				e.title("Now Playing")
					.field(
						"Track:",
						format!(
							"[{}]({})",
							chop_str(track_info.title.as_str(), MAX_SINGLE_ENTRY_LENGTH),
							track_info.uri,
						),
						false,
					)
					.field(
						"Requested By:",
						UserId(
							now_playing
								.requester
								.expect("Expected a requester associated with a playing track")
								.0,
						)
						.mention(),
						false,
					)
					.field(
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
					)
			})
			.await?;
			something_playing = true;
		}
	}
	if !something_playing {
		reply(ctx, "Nothing is playing at the moment.").await?;
	}

	Ok(())
}

/// Show the playback queue.
#[command(slash_command, aliases("q"))]
pub async fn queue(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = if let Some(guild) = ctx.guild() {
		guild
	} else {
		reply(ctx, "You must use this command from within a server.").await?;
		return Ok(());
	};

	let lava_client = &ctx.data().lavalink;

	let mut something_in_queue = false;
	if let Some(node) = lava_client.nodes().await.get(&guild.id.0) {
		let queue = &node.queue;
		let queue_len = queue.len();

		if queue_len > 0 {
			something_in_queue = true;

			let global_queued_count = {
				let guild_id = ctx.guild().unwrap().id;
				let mut hash_map = ctx.data().queued_count.lock().unwrap();
				*hash_map.entry(guild_id).or_default()
			};
			let entry_offset = global_queued_count - queue_len;
			let number_width = global_queued_count.log10() as usize + 1;

			let mut desc = String::new();
			for (i, queued_track) in queue.iter().enumerate() {
				let track_info = queued_track.track.info.as_ref().unwrap();
				desc.push_str(format!("`{:01$}.` [", entry_offset + i + 1, number_width).as_str());
				push_chopped_str(&mut desc, track_info.title.as_str(), MAX_LIST_ENTRY_LENGTH);
				desc.push_str("](");
				desc.push_str(track_info.uri.as_str());
				desc.push(')');
				if i < queue_len - 1 {
					desc.push('\n');
					if desc.len() > DESCRIPTION_LENGTH_CUTOFF {
						desc.push_str("*…the rest has been clipped*");
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
