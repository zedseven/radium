use anyhow::{Context, Error};
use poise::{send_reply, serenity::builder::CreateEmbed};

use crate::{constants::MAIN_COLOUR, PoiseContext};

pub async fn reply<S: ToString>(ctx: PoiseContext<'_>, msg: S) -> Result<(), Error> {
	send_reply(ctx, |m| m.embed(|e| e.colour(MAIN_COLOUR).description(msg)))
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

pub fn push_chopped_str(base: &mut String, new_str: &str, max_len: usize) {
	const ELLIPSIS: char = '…';

	if new_str.len() > max_len {
		base.push_str(&new_str[0..(max_len - 1)]);
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
