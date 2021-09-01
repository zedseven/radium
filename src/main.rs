use std::{collections::HashSet, env::var, time::Duration};

use anyhow::Context;
use lavalink_rs::{
	gateway::LavalinkEventHandler,
	model::{TrackFinish, TrackStart},
	LavalinkClient,
};
use poise::{
	command,
	defaults::{on_error, register_slash_commands},
	say_reply,
	serenity::{
		self,
		async_trait,
		client::parse_token,
		http::Http,
		model::{gateway::Ready, id::ApplicationId, misc::Mentionable},
		prelude::TypeMapKey,
		Client,
	},
	BoxFuture,
	Event,
	Framework,
};
use songbird::SerenityInit;

type Error = Box<dyn std::error::Error + Send + Sync>;
type PoiseContext<'a> = poise::Context<'a, Data, Error>;
type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
type SerenityContext = serenity::client::Context;

struct Data;

struct Lavalink;

impl TypeMapKey for Lavalink {
	type Value = LavalinkClient;
}

struct LavalinkHandler;

#[async_trait]
impl LavalinkEventHandler for LavalinkHandler {
	/*async fn track_start(&self, _client: LavalinkClient, event: TrackStart) {
		println!("Track started!\nGuild: {}", event.guild_id);
	}
	async fn track_finish(&self, _client: LavalinkClient, event: TrackFinish) {
		println!("Track finished!\nGuild: {}", event.guild_id);
	}*/
}

const TOKEN_VAR: &str = "DISCORD_TOKEN";
const LAVALINK_HOST_VAR: &str = "LAVALINK_HOST";
const LAVALINK_PASSWORD_VAR: &str = "LAVALINK_PASSWORD_VAR";

async fn reply<S: Into<String>>(ctx: PoiseContext<'_>, msg: S) -> Result<(), anyhow::Error> {
	say_reply(ctx, msg.into())
		.await
		.with_context(|| "Failed to send message")
}

/// Event Handler
fn listener<'a, U, E: Send>(
	ctx: &'a SerenityContext,
	event: &'a Event<'a>,
	_framework: &'a Framework<U, E>,
	_data: &'a U,
) -> BoxFuture<'a, Result<(), E>> {
	match event {
		Event::Ready { data_about_bot } => Box::pin(async move {
			ready(ctx, data_about_bot).await;
			Ok(())
		}),
		_ => Box::pin(std::future::ready(Ok(()))),
	}
}

/// Startup Function
async fn ready(ctx: &SerenityContext, ready: &Ready) {
	println!("{} is connected!", ready.user.name);
	if ready.guilds.is_empty() {
		println!("No connected guilds.");
		return;
	}
	println!("Connected guilds:");
	for guild in &ready.guilds {
		let guild_data = guild
			.id()
			.to_partial_guild(&ctx.http)
			.await
			.unwrap_or_else(|_| panic!("Unable to get guild with id {}", guild.id()));
		println!(
			"{} - {} ({})",
			guild.id().0,
			guild_data.name,
			guild_data.approximate_member_count.unwrap_or(0)
		);
	}
}

/// Register slash commands in this server or globally.
///
/// Run with no arguments to register globally, run with argument "local" to
/// register in-server.
#[command(owners_only, hide_in_help)]
async fn register(ctx: PrefixContext<'_>, #[flag] local: bool) -> Result<(), Error> {
	register_slash_commands(ctx, !local)
		.await
		.with_context(|| "Failed to register slash commands".to_owned())?;
	Ok(())
}

/// Ping Radium.
#[command(slash_command)]
async fn ping(ctx: PoiseContext<'_>) -> Result<(), Error> {
	reply(ctx, "Pong!").await?;
	Ok(())
}

/// Have Radium join the voice channel you're in.
#[command(slash_command, aliases("j"))]
async fn join(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server").await?;
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
			reply(ctx, "You must use this command while in a voice channel").await?;
			return Ok(());
		}
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

	let (_, handler) = manager.join_gateway(guild.id, channel_id).await;

	match handler {
		Ok(connection_info) => {
			let data = ctx.discord().data.read().await;
			let lava_client = data.get::<Lavalink>().unwrap().clone();
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
async fn leave(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server").await?;
			return Ok(());
		}
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

	if manager.get(guild.id).is_some() {
		if let Err(e) = manager.remove(guild.id).await {
			reply(ctx, format!("Error leaving voice channel: {}", e)).await?;
			return Ok(());
		}

		{
			let data = ctx.discord().data.read().await;
			let lava_client = data.get::<Lavalink>().unwrap().clone();
			lava_client.destroy(guild.id.0).await?;
		}

		reply(ctx, "Left the voice channel").await?;
	} else {
		reply(ctx, "Not in a voice channel").await?;
	}

	Ok(())
}

/// Play something.
#[command(slash_command, aliases("p"))]
async fn play(
	ctx: PoiseContext<'_>,
	#[rest]
	#[description = "What to play"]
	query: String,
) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server").await?;
			return Ok(());
		}
	};

	let manager = songbird::get(ctx.discord()).await.unwrap().clone();

	if let Some(_handler) = manager.get(guild.id) {
		let data = ctx.discord().data.read().await;
		let lava_client = data.get::<Lavalink>().unwrap().clone();

		let query_information = lava_client.auto_search_tracks(&query).await?;

		if query_information.tracks.is_empty() {
			reply(ctx, "Could not find anything for the search query").await?;
			return Ok(());
		}

		if let Err(e) = &lava_client
			.play(guild.id.0, query_information.tracks[0].clone())
			// Change this to play() if you want your own custom queue or no queue at all.
			.queue()
			.await
		{
			eprintln!("{}", e);
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
		reply(ctx, "Radium must be in a voice channel first").await?;
	}

	Ok(())
}

/// Skip the current track.
#[command(slash_command, aliases("next"))]
async fn skip(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server").await?;
			return Ok(());
		}
	};

	let data = ctx.discord().data.read().await;
	let lava_client = data.get::<Lavalink>().unwrap().clone();

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
async fn now_playing(ctx: PoiseContext<'_>) -> Result<(), Error> {
	let guild = match ctx.guild() {
		Some(guild) => guild,
		None => {
			reply(ctx, "You must use this command from within a server").await?;
			return Ok(());
		}
	};

	let data = ctx.discord().data.read().await;
	let lava_client = data.get::<Lavalink>().unwrap().clone();

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
		reply(ctx, "Nothing is playing at the moment").await?;
	}

	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	let token = var(TOKEN_VAR).with_context(|| {
		format!(
			"Expected the discord token in the environment variable {}",
			TOKEN_VAR
		)
	})?;
	let app_id = parse_token(&token)
		.with_context(|| "Token is invalid".to_owned())?
		.bot_user_id;

	let http = Http::new_with_token(&token);
	let owner_id = http
		.get_current_application_info()
		.await
		.with_context(|| "Failed to get application info".to_owned())?
		.owner
		.id;

	println!("Application ID: {}", app_id);
	println!("Owner ID: {}", owner_id);

	let mut owners = HashSet::new();
	owners.insert(owner_id);
	let mut options = poise::FrameworkOptions {
		listener,
		prefix_options: poise::PrefixFrameworkOptions {
			edit_tracker: Some(poise::EditTracker::for_timespan(Duration::from_secs(3600))),
			..Default::default()
		},
		on_error: |e, ctx| Box::pin(on_error(e, ctx)),
		owners,
		..Default::default()
	};

	options.command(register(), |f| f);
	options.command(ping(), |f| f);
	options.command(join(), |f| f);
	options.command(leave(), |f| f);
	options.command(play(), |f| f);
	options.command(skip(), |f| f);
	options.command(now_playing(), |f| f);

	let framework = Framework::new(
		"-".to_owned(),
		ApplicationId(app_id.0),
		move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data) }),
		options,
	);

	let lava_client = LavalinkClient::builder(app_id.0)
		.set_host(var(LAVALINK_HOST_VAR).unwrap_or_else(|_| "127.0.0.1".to_owned()))
		.set_password(var(LAVALINK_PASSWORD_VAR).with_context(|| {
			format!(
				"Expected the Lavalink password in the environment variable {}",
				LAVALINK_PASSWORD_VAR
			)
		})?)
		.build(LavalinkHandler)
		.await
		.with_context(|| "Failed to start the Lavalink client")?;

	let client_builder = Client::builder(&token)
		.register_songbird()
		.type_map_insert::<Lavalink>(lava_client);

	framework
		.start(client_builder)
		.await
		.with_context(|| "Failed to start up".to_owned())?;

	Ok(())
}
