// Uses
use serenity::{
	async_trait,
	client::RawEventHandler,
	model::{event::Event, gateway::Ready},
};
use yansi::Paint;

use crate::{
	constants::{ERROR_STYLE, OKAY_STYLE},
	SerenityContext,
	HEADER_STYLE,
};

// The event handler for all Serenity events
pub struct SerenityHandler;

#[async_trait]
#[allow(clippy::single_match, clippy::wildcard_enum_match_arm)]
impl RawEventHandler for SerenityHandler {
	async fn raw_event(&self, ctx: SerenityContext, event: Event) {
		match event {
			Event::Ready(ready) => on_ready(ctx, ready.ready).await,
			_ => (),
		}
	}
}

/// Startup Function.
async fn on_ready(ctx: SerenityContext, ready: Ready) {
	println!(
		"{}",
		format!("{} is connected!", ready.user.name).paint(OKAY_STYLE)
	);
	if ready.guilds.is_empty() {
		println!("{}", "No connected guilds.".paint(ERROR_STYLE));
		return;
	}
	println!("{}", "Connected guilds:".paint(HEADER_STYLE));
	for guild in &ready.guilds {
		let guild_data = guild
			.id
			.to_partial_guild(&ctx.http)
			.await
			.unwrap_or_else(|_| panic!("unable to get guild with id {}", guild.id));
		println!("{} - {}", guild.id.0, guild_data.name);
	}
}
