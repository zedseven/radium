use anyhow::Context;
use poise::{
	command,
	defaults::{help as poise_help, register_slash_commands, HelpResponseMode},
	serenity::model::misc::Mentionable,
};

use crate::{
	constants::{CREATED_DATE, CREATOR_ID, PREFIX, SOURCE_LINK},
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

/// Information about available commands.
#[command(slash_command)]
pub async fn help(
	ctx: PoiseContext<'_>,
	#[description = "A specific command to show help about."] command: Option<String>,
) -> Result<(), Error> {
	poise_help(
		ctx,
		command.as_deref(),
		format!(
			"You can also use commands with a '{0}' instead of a slash, eg. '{0}help' instead of \
			 '/help'.",
			PREFIX
		)
		.as_str(),
		HelpResponseMode::Ephemeral,
	)
	.await?;
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
