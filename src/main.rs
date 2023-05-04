// Linting Rules
#![warn(
	clippy::complexity,
	clippy::correctness,
	clippy::pedantic,
	clippy::perf,
	clippy::style,
	clippy::suspicious,
	clippy::clone_on_ref_ptr,
	clippy::dbg_macro,
	clippy::decimal_literal_representation,
	clippy::exit,
	clippy::filetype_is_file,
	clippy::if_then_some_else_none,
	clippy::non_ascii_literal,
	clippy::self_named_module_files,
	clippy::str_to_string,
	clippy::undocumented_unsafe_blocks,
	clippy::wildcard_enum_match_arm
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

// Macro Imports
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;

// Modules
mod commands;
mod constants;
mod db;
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
use diesel::{
	r2d2::{ConnectionManager, Pool},
	SqliteConnection,
};
use dotenv::dotenv;
use lavalink_rs::LavalinkClient;
use poise::{builtins::on_error, EditTracker, Framework, FrameworkOptions, PrefixFrameworkOptions};
use serenity::{
	self,
	http::Http,
	model::{gateway::GatewayIntents, id::GuildId},
	utils::parse_token,
};
use songbird::{SerenityInit, Songbird};
use sponsor_block::Client as SponsorBlockClient;
use yansi::Paint;

use crate::{
	commands::commands,
	constants::{COMMIT_NUMBER_CHOP_LENGTH, HEADER_STYLE, PREFIX, PROGRAM_COMMIT, PROGRAM_VERSION},
	db::init as database_init,
	event_handlers::{LavalinkHandler, SerenityHandler},
	segments::SegmentData,
};

// Runtime Constants
const DISCORD_TOKEN_VAR: &str = "DISCORD_TOKEN";
const DATABASE_URL_VAR: &str = "DATABASE_URL";
const DATABASE_URL_DEFAULT: &str = "db.sqlite";
const LAVALINK_HOST_VAR: &str = "LAVALINK_HOST";
const LAVALINK_PASSWORD_VAR: &str = "LAVALINK_PASSWORD";
const LAVALINK_HOST_DEFAULT: &str = "127.0.0.1";
const SPONSOR_BLOCK_USER_ID_VAR: &str = "SPONSOR_BLOCK_USER_ID";
const DISABLE_CLI_COLOURS_VAR: &str = "DISABLE_CLI_COLOURS";

// Definitions
pub type DataArc = Arc<Data>;
pub type Error = Box<dyn error::Error + Send + Sync>;
pub type PoiseContext<'a> = poise::Context<'a, DataArc, Error>;
pub type PoisePrefixContext<'a> = poise::PrefixContext<'a, DataArc, Error>;
pub type SerenityContext = serenity::client::Context;

pub struct Data {
	db_pool:       Pool<ConnectionManager<SqliteConnection>>,
	songbird:      Arc<Songbird>,
	lavalink:      LavalinkClient,
	sponsor_block: SponsorBlockClient,
	queued_count:  Mutex<HashMap<GuildId, usize>>,
	segment_data:  Mutex<SegmentData>,
}

/// Entry point.
#[tokio::main]
async fn main() -> Result<(), Error> {
	// Load environment variables
	dotenv().ok();

	// Terminal Colouring Stuff
	if var(DISABLE_CLI_COLOURS_VAR).is_ok() {
		Paint::disable();
	} else {
		Paint::enable_windows_ascii();
	}

	// Header
	println!(
		"{}",
		HEADER_STYLE.paint(format!(
			"{} --- {} --- {}",
			Paint::yellow("\u{2622}\u{fe0f}"),
			Paint::red(format!("Radium v{PROGRAM_VERSION}")),
			Paint::green("\u{1f4fb}")
		))
	);

	// Prepare basic bot information
	let raw_token = var(DISCORD_TOKEN_VAR).with_context(|| {
		format!("expected the discord token in the environment variable {DISCORD_TOKEN_VAR}")
	})?;
	let token = if raw_token.starts_with("Bot ") || raw_token.starts_with("Bearer ") {
		raw_token.to_string()
	} else {
		format!("Bot {raw_token}")
	};
	let app_id = parse_token(&raw_token)
		.with_context(|| "token is invalid".to_owned())?
		.0;

	let sponsor_block_user_id = var(SPONSOR_BLOCK_USER_ID_VAR).with_context(|| {
		format!(
			"expected the SponsorBlock user ID in the environment variable \
			 {SPONSOR_BLOCK_USER_ID_VAR}"
		)
	})?;

	let http = Http::new(&token);
	let owner_id = http
		.get_current_application_info()
		.await
		.with_context(|| "failed to get application info".to_owned())?
		.owner
		.id;

	println!(
		"{}     {}",
		HEADER_STYLE.paint("Build Commit:"),
		&PROGRAM_COMMIT[..COMMIT_NUMBER_CHOP_LENGTH]
	);
	println!("{}   {}", HEADER_STYLE.paint("Application ID:"), app_id);
	println!("{}         {}", HEADER_STYLE.paint("Owner ID:"), owner_id);

	let mut owners = HashSet::new();
	owners.insert(owner_id);
	let options = FrameworkOptions {
		commands: commands(),
		prefix_options: PrefixFrameworkOptions {
			prefix: Some(PREFIX.to_owned()),
			mention_as_prefix: true,
			case_insensitive_commands: true,
			edit_tracker: Some(EditTracker::for_timespan(Duration::from_secs(3600))),
			..PrefixFrameworkOptions::default()
		},
		on_error: |e| {
			Box::pin(async {
				on_error(e)
					.await
					.expect("Poise's builtin error handler encountered an error");
			})
		},
		owners,
		..FrameworkOptions::default()
	};

	// Start up the bot

	// This mess is so that we can give the Lavalink event handler access to the
	// global Data which we don't actually have initialized yet
	let pre_init_data_arc = Arc::new(Mutex::new(None));

	let lava_client = LavalinkClient::builder(app_id.0)
		.set_host(var(LAVALINK_HOST_VAR).unwrap_or_else(|_| LAVALINK_HOST_DEFAULT.to_owned()))
		.set_password(var(LAVALINK_PASSWORD_VAR).with_context(|| {
			format!(
				"expected the Lavalink password in the environment variable \
				 {LAVALINK_PASSWORD_VAR}"
			)
		})?)
		.build(LavalinkHandler {
			data: Arc::clone(&pre_init_data_arc),
		})
		.await
		.with_context(|| "failed to start the Lavalink client")?;
	let sponsor_block_client = SponsorBlockClient::builder(sponsor_block_user_id).build();
	// Query the SponsorBlock API for the revision number and to test if it's
	// operational
	print!("{} ", HEADER_STYLE.paint("SponsorBlock API:"));
	match sponsor_block_client
		.fetch_api_status()
		.await
		.ok()
		.map(|api_status| api_status.commit)
	{
		Some(commit) => println!("{}", &commit[..COMMIT_NUMBER_CHOP_LENGTH]),
		None => println!("Unknown"),
	}

	let database_pool =
		database_init(var(DATABASE_URL_VAR).unwrap_or_else(|_| DATABASE_URL_DEFAULT.to_owned()))
			.with_context(|| "failed to initialize the database")?;

	let songbird = Songbird::serenity();
	let songbird_clone = Arc::clone(&songbird); // Required because the closure that uses it moves the value

	let data = Arc::new(Data {
		db_pool:       database_pool,
		songbird:      songbird_clone,
		lavalink:      lava_client,
		sponsor_block: sponsor_block_client,
		queued_count:  Mutex::new(HashMap::new()),
		segment_data:  Mutex::new(SegmentData::new()),
	});
	// Set the Data Arc that was given to the LavalinkHandler
	{
		let mut data_guard = pre_init_data_arc.lock().unwrap();
		*data_guard = Some(Arc::clone(&data));
	}

	Framework::builder()
		.options(options)
		.token(&token)
		.intents(GatewayIntents::non_privileged() | GatewayIntents::MESSAGE_CONTENT)
		.client_settings(|client_builder| {
			client_builder
				.raw_event_handler(SerenityHandler)
				.register_songbird_with(songbird)
		})
		.setup(move |_ctx, _ready, _framework| Box::pin(async move { Ok(data) }))
		.build()
		.await
		.with_context(|| "failed to build the bot framework")?
		.start()
		.await
		.with_context(|| "failed to start up")?;

	Ok(())
}
