use anyhow::Context;
use lavalink_rs::model::Track;
use poise::{command, serenity::model::misc::Mentionable};
use url::Url;

use crate::{
	util::{chop_str, push_chopped_str, reply, reply_embed},
	Error,
	PoiseContext,
};

const MAX_DESCRIPTION_LENGTH: usize = 2048;
const DESCRIPTION_LENGTH_CUTOFF: usize = MAX_DESCRIPTION_LENGTH - 512;
const MAX_LIST_ENTRY_LENGTH: usize = 60;
const MAX_SINGLE_ENTRY_LENGTH: usize = 40;
const UNKNOWN_TITLE: &str = "Unknown title";

/// Have Radium join the voice channel you're in.
#[command(slash_command, aliases("j"))]
pub async fn join(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server.").await?;
			return Ok(());
		}
	};

	let channel_id = match guild
		.voice_states
		.get(&ctx.author().id)
		.and_then(|voice_state| voice_state.channel_id)
	{
		Some(channel) => channel,
		None => {
			reply(ctx, "You must use this command while in a voice channel.").await?;
			return Ok(());
		}
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

	let (_, handler) = manager.join_gateway(guild.id, channel_id).await;

	match handler {
		Ok(connection_info) => {
			let lava_client = &ctx.data().lavalink;
			lava_client.create_session(&connection_info).await?;

			reply(ctx, format!("Joined {}", channel_id.mention())).await?;
		}
		Err(e) => {
			reply(
				ctx,
				format!("Error joining {}: {}", channel_id.mention(), e),
			)
			.await?
		}
	}

	Ok(())
}

/// Have Radium leave the voice channel it's in, if any.
#[command(slash_command, aliases("l"))]
pub async fn leave(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server.").await?;
			return Ok(());
		}
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

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

/// Play something.
#[command(slash_command, aliases("p"))]
pub async fn play(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What to play."]
	query: String,
) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server.").await?;
			return Ok(());
		}
	};

	let attachments = match ctx {
		PoiseContext::Prefix(prefix_ctx) => prefix_ctx
			.msg
			.attachments
			.iter()
			.filter(|a| match &a.content_type {
				Some(t) => t.starts_with("audio") || t.starts_with("video"),
				None => false,
			})
			.map(|a| (a.url.clone(), a.filename.clone()))
			.collect::<Vec<(String, String)>>(),
		PoiseContext::Slash(_) => Vec::new(),
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

	if let Some(_handler) = manager.get(guild.id) {
		let lava_client = &ctx.data().lavalink;

		let mut query_results = Vec::new();

		// Load up any attachments
		for attachment in attachments {
			let mut query_result = lava_client.auto_search_tracks(&attachment.0).await?;
			for t in &mut query_result.tracks {
				t.info = match &t.info {
					Some(old_info) => {
						let mut new_info = old_info.clone();
						new_info.title = attachment.1.clone();
						Some(new_info)
					}
					None => None,
				}
			}
			query_results.extend_from_slice(&query_result.tracks)
		}

		// Load the command query
		let query_information = lava_client.auto_search_tracks(&query).await?;

		let is_url = Url::parse(query.trim()).is_ok();

		// If the query was a URL, then it's likely a playlist where all retrieved
		// tracks are desired - otherwise, it's just the top result
		let query_tracks = if is_url {
			query_information.tracks.len()
		} else {
			1
		};

		query_results.extend_from_slice(
			&query_information
				.tracks
				.iter()
				.take(query_tracks)
				.cloned()
				.collect::<Vec<Track>>(),
		);

		if query_results.is_empty() {
			reply(ctx, "Could not find anything for the search query.").await?;
			return Ok(());
		}

		let query_results_len = query_results.len();

		// For URLs that point to raw files, Lavalink seems to just return them with a
		// title of "Unknown title" - this is a slightly hacky solution to set the title
		// to the filename of the raw file
		if is_url && query_tracks == 1 {
			let track_info = &mut query_results[query_results_len - 1];
			if track_info.info.is_some()
				&& track_info
					.info
					.as_ref()
					.unwrap()
					.title
					.as_str()
					.eq(UNKNOWN_TITLE)
			{
				track_info.info = match &track_info.info {
					Some(old_info) => {
						let mut new_info = old_info.clone();
						new_info.title = Url::parse(old_info.uri.as_str())
							.expect(
								"Unable to parse track info URI when it should have been \
								 guaranteed to be valid",
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
		for track in &query_results {
			if let Err(e) = lava_client.play(guild.id.0, track.clone()).queue().await {
				reply(ctx, "Failed to queue up query result.").await?;
				eprintln!("Failed to queue up query result: {}", e);
				return Ok(());
			};
		}

		// Notify the user of the added tracks
		if query_results_len == 1 {
			let track_info = query_results[0].info.as_ref().unwrap();
			reply(
				ctx,
				format!(
					"Added to queue: [{}]({}) [{}]",
					chop_str(track_info.title.as_str(), MAX_SINGLE_ENTRY_LENGTH),
					track_info.uri,
					ctx.author().mention()
				),
			)
			.await?;
		} else {
			let mut description = String::from("Requested by ");
			description.push_str(ctx.author().mention().to_string().as_str());
			description.push('\n');
			for (i, track) in query_results.iter().enumerate() {
				let track_info = track.info.as_ref().unwrap();
				description.push_str("- [");
				push_chopped_str(
					&mut description,
					track_info.title.as_str(),
					MAX_LIST_ENTRY_LENGTH,
				);
				description.push_str("](");
				description.push_str(track_info.uri.as_str());
				description.push(')');
				if i < query_results_len - 1 {
					description.push('\n');
					if description.len() > DESCRIPTION_LENGTH_CUTOFF {
						description.push_str("*…the rest has been clipped*");
						break;
					}
				}
			}
			reply_embed(ctx, |e| {
				e.title(format!("Added {} Tracks:", query_results_len))
					.description(description)
			})
			.await?;
		}
	} else {
		reply(ctx, "Radium must be in a voice channel first.").await?;
	}

	Ok(())
}

/// Skip the current track.
#[command(slash_command, aliases("next", "stop"))]
pub async fn skip(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server.").await?;
			return Ok(());
		}
	};

	let lava_client = &ctx.data().lavalink;

	if let Some(track) = lava_client.skip(guild.id.0).await {
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
			format!("Skipped: {}", track.track.info.as_ref().unwrap().title),
		)
		.await?;
	} else {
		reply(ctx, "Nothing to skip.").await?;
	}

	Ok(())
}

/// Show what's currently playing.
#[command(slash_command, aliases("nowplaying", "np", "current"))]
pub async fn now_playing(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server.").await?;
			return Ok(());
		}
	};

	let lava_client = &ctx.data().lavalink;

	let mut something_playing = false;
	if let Some(node) = lava_client.nodes().await.get(&guild.id.0) {
		if let Some(track) = &node.now_playing {
			reply(
				ctx,
				format!("Now Playing: {}", track.track.info.as_ref().unwrap().title),
			)
			.await?;
			something_playing = true;
		}
	}
	if !something_playing {
		reply(ctx, "Nothing is playing at the moment.").await?;
	}

	Ok(())
}
