use anyhow::Context;
use poise::say_reply;

use crate::PoiseContext;

pub async fn reply<S: Into<String>>(ctx: PoiseContext<'_>, msg: S) -> Result<(), anyhow::Error> {
	say_reply(ctx, msg.into())
		.await
		.with_context(|| "Failed to send message")
}
