use anyhow::Context;
use poise::{command, defaults::register_slash_commands};

use crate::{util::reply, Error, PoiseContext, PrefixContext};

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

/// Ping Radium.
#[command(slash_command)]
pub async fn ping(ctx: PoiseContext<'_>) -> Result<(), Error> {
	reply(ctx, "Pong!").await?;
	Ok(())
}
