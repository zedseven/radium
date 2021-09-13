#![feature(int_log)]
#![allow(dead_code)]

mod commands;
mod constants;
mod util;

use std::{
	collections::{HashMap, HashSet},
	env::var,
	error,
	sync::{Arc, Mutex},
	time::Duration,
};

use anyhow::Context;
use lavalink_rs::{gateway::LavalinkEventHandler, LavalinkClient};
use poise::{
	defaults::on_error,
	serenity::{
		self,
		async_trait,
		client::{parse_token, RawEventHandler},
		http::Http,
		model::{
			event::Event,
			gateway::Ready,
			id::{ApplicationId, GuildId},
		},
		Client,
	},
	Framework,
};
use songbird::{SerenityInit, Songbird};

use crate::{commands::*, constants::PREFIX};

// Runtime Constants
const TOKEN_VAR: &str = "DISCORD_TOKEN";
const LAVALINK_HOST_VAR: &str = "LAVALINK_HOST";
const LAVALINK_PASSWORD_VAR: &str = "LAVALINK_PASSWORD";
const LAVALINK_HOST_DEFAULT: &str = "127.0.0.1";

// Definitions
pub type Error = Box<dyn error::Error + Send + Sync>;
pub type PoiseContext<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type SerenityContext = serenity::client::Context;

pub struct Data {
	songbird: Arc<Songbird>,
	lavalink: LavalinkClient,
	queued_count: Mutex<HashMap<GuildId, usize>>,
}

struct Handler;
struct LavalinkHandler;

/// Event Handlers
#[async_trait]
#[allow(clippy::single_match)]
impl RawEventHandler for Handler {
	async fn raw_event(&self, ctx: SerenityContext, event: Event) {
		match event {
			Event::Ready(ready) => on_ready(ctx, ready.ready).await,
			_ => (),
		}
	}
}

#[async_trait]
impl LavalinkEventHandler for LavalinkHandler {
	/*async fn track_start(&self, _client: LavalinkClient, event: TrackStart) {
		println!("Track started!\nGuild: {}", event.guild_id);
	}
	async fn track_finish(&self, _client: LavalinkClient, event: TrackFinish) {
		println!("Track finished!\nGuild: {}", event.guild_id);
	}*/
}

/// Entry point.
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
		prefix_options: poise::PrefixFrameworkOptions {
			edit_tracker: Some(poise::EditTracker::for_timespan(Duration::from_secs(3600))),
			..Default::default()
		},
		on_error: |e, ctx| Box::pin(on_error(e, ctx)),
		owners,
		..Default::default()
	};

	options.command(register(), |f| f);
	options.command(about(), |f| f);
	options.command(ping(), |f| f);
	options.command(join(), |f| f);
	options.command(leave(), |f| f);
	options.command(play(), |f| f);
	options.command(skip(), |f| f);
	options.command(clear(), |f| f);
	options.command(now_playing(), |f| f);
	options.command(queue(), |f| f);
	options.command(roll(), |f| f);

	let lava_client = LavalinkClient::builder(app_id.0)
		.set_host(var(LAVALINK_HOST_VAR).unwrap_or_else(|_| LAVALINK_HOST_DEFAULT.to_owned()))
		.set_password(var(LAVALINK_PASSWORD_VAR).with_context(|| {
			format!(
				"Expected the Lavalink password in the environment variable {}",
				LAVALINK_PASSWORD_VAR
			)
		})?)
		.build(LavalinkHandler)
		.await
		.with_context(|| "Failed to start the Lavalink client")?;

	let songbird = Songbird::serenity();
	let songbird_clone = songbird.clone(); // Required because the closure that uses it moves the value
	let framework = Framework::new(
		PREFIX.to_owned(),
		ApplicationId(app_id.0),
		move |_ctx, _ready, _framework| {
			Box::pin(async move {
				Ok(Data {
					songbird: songbird_clone,
					lavalink: lava_client,
					queued_count: Mutex::new(HashMap::new()),
				})
			})
		},
		options,
	);

	framework
		.start(
			Client::builder(&token)
				.raw_event_handler(Handler)
				.register_songbird_with(songbird),
		)
		.await
		.with_context(|| "Failed to start up".to_owned())?;

	Ok(())
}

/// Startup Function.
async fn on_ready(ctx: SerenityContext, ready: Ready) {
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
		println!("{} - {}", guild.id().0, guild_data.name);
	}
}
