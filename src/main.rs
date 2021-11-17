// Features
#![feature(label_break_value)]
#![feature(int_log)]
// Linting Rules
#![warn(
	clippy::complexity,
	clippy::correctness,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::pedantic,
	clippy::filetype_is_file,
	clippy::str_to_string
)]
#![allow(
	clippy::cast_possible_truncation,
	clippy::cast_possible_wrap,
	clippy::cast_precision_loss,
	clippy::cast_sign_loss,
	clippy::doc_markdown,
	clippy::module_name_repetitions,
	clippy::no_effect_underscore_binding,
	clippy::similar_names,
	clippy::too_many_lines,
	clippy::unnecessary_wraps,
	clippy::wildcard_imports,
	dead_code,
	unused_macros
)]

// Modules
mod commands;
mod constants;
mod event_handlers;
mod segments;
mod util;

// Uses
use std::{
	collections::{HashMap, HashSet},
	env::var,
	error,
	sync::{Arc, Mutex},
	time::Duration,
};

use anyhow::Context;
use dotenv::dotenv;
use lavalink_rs::LavalinkClient;
use poise::{
	builtins::on_error,
	serenity::{self, client::parse_token, http::Http, model::id::GuildId},
	EditTracker,
	Framework,
	FrameworkOptions,
	PrefixFrameworkOptions,
};
use songbird::{SerenityInit, Songbird};
use sponsor_block::Client as SponsorBlockClient;

use crate::{
	commands::*,
	constants::{PREFIX, PROGRAM_VERSION},
	event_handlers::{LavalinkHandler, SerenityHandler},
	segments::SegmentData,
};

// Runtime Constants
const DISCORD_TOKEN_VAR: &str = "DISCORD_TOKEN";
const LAVALINK_HOST_VAR: &str = "LAVALINK_HOST";
const LAVALINK_PASSWORD_VAR: &str = "LAVALINK_PASSWORD";
const LAVALINK_HOST_DEFAULT: &str = "127.0.0.1";
const SPONSOR_BLOCK_USER_ID_VAR: &str = "SPONSOR_BLOCK_USER_ID";

// Definitions
pub type DataArc = Arc<Data>;
pub type Error = Box<dyn error::Error + Send + Sync>;
pub type PoiseContext<'a> = poise::Context<'a, DataArc, Error>;
pub type PoisePrefixContext<'a> = poise::PrefixContext<'a, DataArc, Error>;
pub type SerenityContext = serenity::client::Context;

pub struct Data {
	songbird: Arc<Songbird>,
	lavalink: LavalinkClient,
	sponsor_block: SponsorBlockClient,
	queued_count: Mutex<HashMap<GuildId, usize>>,
	segment_data: Mutex<SegmentData>,
}

/// Entry point.
#[tokio::main]
async fn main() -> Result<(), Error> {
	println!(
		"\u{2622}\u{fe0f} --- Radium v{} --- \u{1f4fb}",
		PROGRAM_VERSION
	);

	// Load environment variables
	dotenv().ok();

	// Prepare basic bot information
	let token = var(DISCORD_TOKEN_VAR).with_context(|| {
		format!(
			"Expected the discord token in the environment variable {}",
			DISCORD_TOKEN_VAR
		)
	})?;
	let app_id = parse_token(&token)
		.with_context(|| "Token is invalid".to_owned())?
		.bot_user_id;

	let sponsor_block_user_id = var(SPONSOR_BLOCK_USER_ID_VAR).with_context(|| {
		format!(
			"Expected the SponsorBlock user ID in the environment variable {}",
			SPONSOR_BLOCK_USER_ID_VAR
		)
	})?;

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
	let mut options = FrameworkOptions {
		prefix_options: PrefixFrameworkOptions {
			prefix: Some(PREFIX.to_owned()),
			mention_as_prefix: true,
			case_insensitive_commands: true,
			edit_tracker: Some(EditTracker::for_timespan(Duration::from_secs(3600))),
			..PrefixFrameworkOptions::default()
		},
		on_error: |e, ctx| Box::pin(on_error(e, ctx)),
		owners,
		..FrameworkOptions::default()
	};

	// Command Initialization
	// Utility
	options.command(register(), |f| f);
	options.command(help(), |f| f);
	options.command(about(), |f| f);
	options.command(ping(), |f| f);
	// Playback
	options.command(join(), |f| f);
	options.command(leave(), |f| f);
	options.command(play(), |f| f);
	options.command(skip(), |f| f);
	options.command(pause(), |f| f);
	options.command(resume(), |f| f);
	options.command(seek(), |f| f);
	options.command(clear(), |f| f);
	options.command(now_playing(), |f| f);
	options.command(queue(), |f| f);
	// Chance
	options.command(roll(), |f| f);

	// Start up the bot

	// This mess is so that we can give the Lavalink event handler access to the
	// global Data which we don't actually have initialized yet
	let pre_init_data_arc = Arc::new(Mutex::new(None));

	let lava_client = LavalinkClient::builder(app_id.0)
		.set_host(var(LAVALINK_HOST_VAR).unwrap_or_else(|_| LAVALINK_HOST_DEFAULT.to_owned()))
		.set_password(var(LAVALINK_PASSWORD_VAR).with_context(|| {
			format!(
				"Expected the Lavalink password in the environment variable {}",
				LAVALINK_PASSWORD_VAR
			)
		})?)
		.build(LavalinkHandler {
			data: pre_init_data_arc.clone(),
		})
		.await
		.with_context(|| "Failed to start the Lavalink client")?;
	let sponsor_block_client = SponsorBlockClient::new(sponsor_block_user_id);

	let songbird = Songbird::serenity();
	let songbird_clone = songbird.clone(); // Required because the closure that uses it moves the value

	let data = Arc::new(Data {
		songbird: songbird_clone,
		lavalink: lava_client,
		sponsor_block: sponsor_block_client,
		queued_count: Mutex::new(HashMap::new()),
		segment_data: Mutex::new(SegmentData::new()),
	});
	// Set the Data Arc that was given to the LavalinkHandler
	{
		let mut data_guard = pre_init_data_arc.lock().unwrap();
		*data_guard = Some(data.clone());
	}

	Framework::build()
		.options(options)
		.token(&token)
		.client_settings(|client_builder| {
			client_builder
				.raw_event_handler(SerenityHandler)
				.register_songbird_with(songbird)
		})
		.user_data_setup(move |_ctx, _ready, _framework| Box::pin(async move { Ok(data) }))
		.build()
		.await
		.with_context(|| "Failed to build the bot framework")?
		.start()
		.await
		.with_context(|| "Failed to start up")?;

	Ok(())
}
