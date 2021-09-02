use anyhow::Context;
use poise::{command, serenity::model::misc::Mentionable};

use crate::{util::reply, Error, PoiseContext};

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
	#[description = "What to play"]
	query: String,
) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server.").await?;
			return Ok(());
		}
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

	if let Some(_handler) = manager.get(guild.id) {
		let lava_client = &ctx.data().lavalink;

		let query_information = lava_client.auto_search_tracks(&query).await?;

		if query_information.tracks.is_empty() {
			reply(ctx, "Could not find anything for the search query.").await?;
			return Ok(());
		}

		if let Err(e) = lava_client
			.play(guild.id.0, query_information.tracks[0].clone())
			// Change this to play() if you want your own custom queue or no queue at all.
			.queue()
			.await
		{
			reply(ctx, "Failed to queue up query result.").await?;
			eprintln!("Failed to queue up query result: {}", e);
			return Ok(());
		};
		reply(
			ctx,
			format!(
				"Added to queue: {}",
				query_information.tracks[0].info.as_ref().unwrap().title
			),
		)
		.await?;
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
