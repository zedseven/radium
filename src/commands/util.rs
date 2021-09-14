use anyhow::Context;
use poise::{
	command,
	defaults::{help as poise_help, register_slash_commands, HelpResponseMode},
	serenity::model::misc::Mentionable,
};

use crate::{
	constants::{CREATED_DATE, CREATOR_ID, PREFIX, PROGRAM_VERSION, SOURCE_LINK},
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

/// Get information about available commands. Use `/help help` for more info.
///
/// Calling this command with the name of another command will give you a more
/// detailed description of what the command does, and how to use it.
///
/// Of course, if you're seeing this, you already know you can do that.
#[command(slash_command, track_edits)]
pub async fn help(
	ctx: PoiseContext<'_>,
	#[description = "A specific command to show help about."] command: Option<String>,
) -> Result<(), Error> {
	poise_help(
		ctx,
		command.as_deref(),
		format!(
			"You can also use commands with a `{0}` instead of a slash, eg. `{0}help` instead of \
			 `/help`.\nEdit your message to the bot and the bot will edit it's response for this \
			 help dialog.",
			PREFIX
		)
		.as_str(),
		HelpResponseMode::Ephemeral,
	)
	.await?;
	Ok(())
}

/// Get basic information about Radium.
///
/// There isn't much else to say - just use the command.
#[command(slash_command)]
pub async fn about(ctx: PoiseContext<'_>) -> Result<(), Error> {
	reply_embed(ctx, |e| {
		e.title("Radium")
			.description(format!("The Radium Radio bot, `v{}`.", PROGRAM_VERSION))
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
///
/// Perhaps at some point in the future this will display the latency, but for
/// now it's pretty much useless.
///
/// It's sticking around for posterity and as a quick way to test if the bot is
/// operational.
#[command(slash_command)]
pub async fn ping(ctx: PoiseContext<'_>) -> Result<(), Error> {
	reply(ctx, "Pong!").await?;
	Ok(())
}
