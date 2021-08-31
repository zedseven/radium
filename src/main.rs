use std::{env, time::Duration};

use poise::{
	command,
	defaults::{on_error, register_slash_commands},
	say_reply,
	serenity,
	serenity::{
		client::parse_token,
		http::Http,
		model::{
			gateway::Ready,
			id::{ApplicationId, UserId},
		},
		Client,
	},
	BoxFuture,
	Event,
	Framework,
};

type Error = Box<dyn std::error::Error + Send + Sync>;
type PoiseContext<'a> = poise::Context<'a, Data, Error>;
type PrefixContext<'a> = poise::PrefixContext<'a, Data, Error>;
type SerenityContext = serenity::client::Context;

struct Data {
	owner_id: UserId,
}

const TOKEN_NAME: &str = "DISCORD_TOKEN";

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
			.expect(format!("Unable to get guild with id {}", guild.id()).as_str());
		println!(
			"{} - {} ({})",
			guild.id().0,
			guild_data.name,
			guild_data.approximate_member_count.unwrap_or(0)
		);
	}
}

async fn is_owner(ctx: PrefixContext<'_>) -> Result<bool, Error> {
	Ok(ctx.msg.author.id == ctx.data.owner_id)
}

/// Register
#[command(check = "is_owner", hide_in_help)]
async fn register(ctx: PrefixContext<'_>, #[flag] global: bool) -> Result<(), Error> {
	register_slash_commands(ctx, global).await?;
	Ok(())
}

/// Ping
#[command(slash_command)]
async fn ping(ctx: PoiseContext<'_>) -> Result<(), Error> {
	say_reply(ctx, "Pong".to_owned()).await?;
	Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
	let token = env::var(TOKEN_NAME).expect(
		format!(
			"Expected the discord token in the environment variable {}",
			TOKEN_NAME
		)
		.as_str(),
	);
	let app_id = parse_token(&token).expect("Token is invalid").bot_user_id;

	let http = Http::new_with_token(&token);
	let owner_id = http.get_current_application_info().await?.owner.id;

	println!("Application ID: {}", app_id);
	println!("Owner ID: {}", owner_id);

	let mut options = poise::FrameworkOptions {
		prefix_options: poise::PrefixFrameworkOptions {
			edit_tracker: Some(poise::EditTracker::for_timespan(Duration::from_secs(3600))),
			..Default::default()
		},
		on_error: |error, ctx| Box::pin(on_error(error, ctx)),
		listener,
		..Default::default()
	};

	options.command(register(), |f| f);
	options.command(ping(), |f| f);

	let framework = Framework::new(
		"-".to_owned(),
		ApplicationId(app_id.0),
		move |_ctx, _ready, _framework| Box::pin(async move { Ok(Data { owner_id }) }),
		options,
	);

	if let Err(e) = framework.start(Client::builder(&token)).await {
		eprintln!("Client error: {:?}", e);
	}

	Ok(())
}
