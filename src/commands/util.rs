use anyhow::Context;
use poise::{command, defaults::register_slash_commands, serenity::model::misc::Mentionable};

use crate::{
	constants::{CREATED_DATE, CREATOR_ID, SOURCE_LINK},
	util::{reply, reply_embed},
	Error,
	PoiseContext,
	PrefixContext,
};

/// Register slash commands in this server or globally.
///
/// Run with no arguments to register globally, run with argument "local" to
/// register in-server.
#[command(owners_only, hide_in_help)]
pub async fn register(ctx: PrefixContext<'_>, #[flag] local: bool) -> Result<(), Error> {
	register_slash_commands(ctx, !local)
		.await
		.with_context(|| "Failed to register slash commands".to_owned())?;
	Ok(())
}

/// Information about Radium.
#[command(slash_command)]
pub async fn about(ctx: PoiseContext<'_>) -> Result<(), Error> {
	reply_embed(ctx, |e| {
		e.title("Radium")
			.description("The Radium Radio bot.")
			.field("Authour", CREATOR_ID.mention(), false)
			.field("Source Link", SOURCE_LINK, false)
			.field(
				"Created",
				format!("{}, because Groovy died. 🚱", CREATED_DATE),
				false,
			)
	})
	.await?;
	Ok(())
}

/// Ping Radium.
#[command(slash_command)]
pub async fn ping(ctx: PoiseContext<'_>) -> Result<(), Error> {
	reply(ctx, "Pong!").await?;
	Ok(())
}
