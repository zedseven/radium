// Uses
use anyhow::{Context, Error};
use lazy_static::lazy_static;
use poise::{send_reply, serenity::builder::CreateEmbed, ReplyHandle};
use regex::Regex;

use crate::{
	constants::{
		MAIN_COLOUR,
		MILLIS_PER_HOUR,
		MILLIS_PER_MINUTE,
		MILLIS_PER_SECOND,
		MINUTES_PER_HOUR,
		SECONDS_PER_HOUR_F32,
		SECONDS_PER_MINUTE,
		SECONDS_PER_MINUTE_F32,
	},
	PoiseContext,
};

// Functions
pub async fn reply<S: ToString>(
	ctx: PoiseContext<'_>,
	msg: S,
) -> Result<Option<ReplyHandle<'_>>, Error> {
	send_reply(ctx, |m| m.embed(|e| e.colour(MAIN_COLOUR).description(msg)))
		.await
		.with_context(|| "failed to send message")
}

pub async fn reply_plain<S: ToString>(
	ctx: PoiseContext<'_>,
	msg: S,
) -> Result<Option<ReplyHandle<'_>>, Error> {
	send_reply(ctx, |m| m.content(msg.to_string()))
		.await
		.with_context(|| "failed to send message")
}

pub async fn reply_embed(
	ctx: PoiseContext<'_>,
	embed: impl FnOnce(&mut CreateEmbed) -> &mut CreateEmbed,
) -> Result<Option<ReplyHandle<'_>>, Error> {
	send_reply(ctx, |m| m.embed(|e| embed(e.colour(MAIN_COLOUR))))
		.await
		.with_context(|| "failed to send message")
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
		.replace('[', "\u{2045}")
		.replace(']', "\u{2046}")
}

pub fn push_chopped_str(base: &mut String, new_str: &str, max_len: usize) {
	const ELLIPSIS: char = '\u{2026}';

	if new_str.len() > max_len {
		base.push_str(escape_str(&new_str[0..(max_len - 1)]).trim_end());
		base.push(ELLIPSIS);
	} else {
		base.push_str(escape_str(new_str).as_str());
	}
}

pub fn chop_str(s: &str, max_len: usize) -> String {
	let mut base = String::new();
	push_chopped_str(&mut base, s, max_len);
	base
}

pub fn none_on_empty(s: &str) -> Option<&str> {
	if s.is_empty() {
		None
	} else {
		Some(s)
	}
}

pub fn is_application_context(ctx: &PoiseContext<'_>) -> bool {
	match ctx {
		PoiseContext::Application(_) => true,
		PoiseContext::Prefix(_) => false,
	}
}

pub fn display_timecode(millis: u64) -> String {
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

pub fn display_timecode_f32(seconds: f32) -> String {
	if seconds >= SECONDS_PER_HOUR_F32 {
		format!(
			"{:02.0}:{:02.0}:{:02.0}",
			(seconds / SECONDS_PER_HOUR_F32).floor(),
			((seconds / SECONDS_PER_MINUTE_F32) % SECONDS_PER_HOUR_F32).floor(),
			(seconds % SECONDS_PER_MINUTE_F32).floor()
		)
	} else {
		format!(
			"{:02.0}:{:02.0}",
			(seconds / SECONDS_PER_MINUTE_F32).floor(),
			(seconds % SECONDS_PER_MINUTE_F32).floor()
		)
	}
}
