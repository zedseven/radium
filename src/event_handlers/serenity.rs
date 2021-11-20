// Uses
use poise::serenity::{
	async_trait,
	client::RawEventHandler,
	model::{event::Event, gateway::Ready},
};

use crate::SerenityContext;

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
