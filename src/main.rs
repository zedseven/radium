mod commands;
mod util;

use std::{collections::HashSet, env::var, error, time::Duration};

use anyhow::Context;
use lavalink_rs::{gateway::LavalinkEventHandler, LavalinkClient};
use poise::{
	defaults::on_error,
	serenity::{
		self,
		async_trait,
		client::parse_token,
		http::Http,
		model::{gateway::Ready, id::ApplicationId},
		prelude::TypeMapKey,
		Client,
	},
	BoxFuture,
	Event,
	Framework,
};
use songbird::SerenityInit;

use crate::commands::*;

pub type Error = Box<dyn error::Error + Send + Sync>;
pub type PoiseContext<'a> = poise::Context<'a, Data, Error>;
pub type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
pub type SerenityContext = serenity::client::Context;

pub struct Data;

pub struct Lavalink;

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
const LAVALINK_PASSWORD_VAR: &str = "LAVALINK_PASSWORD";
const LAVALINK_HOST_DEFAULT: &str = "127.0.0.1";
const PREFIX: &str = "-";

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

/// Startup Function.
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
		println!("{} - {}", guild.id().0, guild_data.name);
	}
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
		PREFIX.to_owned(),
		ApplicationId(app_id.0),
		move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data) }),
		options,
	);

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

	let client_builder = Client::builder(&token)
		.register_songbird()
		.type_map_insert::<Lavalink>(lava_client);

	framework
		.start(client_builder)
		.await
		.with_context(|| "Failed to start up".to_owned())?;

	Ok(())
}
