use anyhow::Context;
use poise::send_reply;

use crate::{PoiseContext, MAIN_COLOUR};

pub async fn reply<S: ToString>(ctx: PoiseContext<'_>, msg: S) -> Result<(), anyhow::Error> {
	send_reply(ctx, |m| m.embed(|e| e.colour(MAIN_COLOUR).description(msg)))
		.await
		.with_context(|| "Failed to send message")
}
