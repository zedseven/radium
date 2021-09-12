use anyhow::{Context, Error};
use lazy_static::lazy_static;
use poise::{send_reply, serenity::builder::CreateEmbed};
use regex::Regex;

use crate::{constants::MAIN_COLOUR, PoiseContext};

pub async fn reply<S: ToString>(ctx: PoiseContext<'_>, msg: S) -> Result<(), Error> {
	send_reply(ctx, |m| m.embed(|e| e.colour(MAIN_COLOUR).description(msg)))
		.await
		.with_context(|| "Failed to send message")
}

pub async fn reply_plain<S: ToString>(ctx: PoiseContext<'_>, msg: S) -> Result<(), Error> {
	send_reply(ctx, |m| m.content(msg.to_string()))
		.await
		.with_context(|| "Failed to send message")
}

pub async fn reply_embed(
	ctx: PoiseContext<'_>,
	embed: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed,
) -> Result<(), Error> {
	send_reply(ctx, |m| m.embed(|e| embed(e.colour(MAIN_COLOUR))))
		.await
		.with_context(|| "Failed to send message")
}

/// Escapes a string for use in Discord, escaping all Markdown characters.
///
/// Square brackets can't be escaped with slashes for some reason, so they're
/// replaced with similar-looking characters.
pub fn escape_str(s: &str) -> String {
	lazy_static! {
		static ref ESCAPE_REGEX: Regex = Regex::new(r"([\\_*~`|])").unwrap();
	}
	ESCAPE_REGEX
		.replace_all(s, r"\$0")
		.replace('[', "⁅")
		.replace(']', "⁆")
}

pub fn push_chopped_str(base: &mut String, new_str: &str, max_len: usize) {
	const ELLIPSIS: char = '…';

	if new_str.len() > max_len {
		base.push_str(escape_str(&new_str[0..(max_len - 1)]).as_str());
		base.push(ELLIPSIS);
	} else {
		base.push_str(new_str);
	}
}

pub fn chop_str(s: &str, max_len: usize) -> String {
	let mut base = String::new();
	push_chopped_str(&mut base, s, max_len);
	base
}

pub fn is_slash_context(ctx: &PoiseContext<'_>) -> bool {
	match ctx {
		PoiseContext::Slash(_) => true,
		PoiseContext::Prefix(_) => false,
	}
}

pub fn display_time_span(millis: u64) -> String {
	const MILLIS_PER_SECOND: u64 = 1000;
	const SECONDS_PER_MINUTE: u64 = 60;
	const MINUTES_PER_HOUR: u64 = 60;
	const MILLIS_PER_MINUTE: u64 = MILLIS_PER_SECOND * SECONDS_PER_MINUTE;
	const MILLIS_PER_HOUR: u64 = MILLIS_PER_MINUTE * MINUTES_PER_HOUR;

	if millis >= MILLIS_PER_HOUR {
		format!(
			"{:02}:{:02}:{:02}",
			millis / MILLIS_PER_HOUR,
			(millis / MILLIS_PER_MINUTE) % MINUTES_PER_HOUR,
			(millis / MILLIS_PER_SECOND) % SECONDS_PER_MINUTE
		)
	} else {
		format!(
			"{:02}:{:02}",
			millis / MILLIS_PER_MINUTE,
			(millis / MILLIS_PER_SECOND) % SECONDS_PER_MINUTE
		)
	}
}
