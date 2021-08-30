use std::env;

use serenity::{
	async_trait,
	model::{channel::Message, gateway::Ready},
	prelude::*,
};

struct Handler;

const TOKEN_NAME: &str = "DISCORD_TOKEN";

#[async_trait]
impl EventHandler for Handler {
	async fn message(&self, ctx: Context, msg: Message) {
		println!("Received message: {}", msg.content);
		if msg.content == "!ping" {
			if let Err(e) = msg.channel_id.say(&ctx.http, "Pong").await {
				eprintln!("Error sending message: {:?}", e);
			}
		}
	}

	async fn ready(&self, _: Context, ready: Ready) {
		println!("{} is connected!", ready.user.name);
		if ready.guilds.is_empty() {
			println!("No connected guilds.");
			return;
		}
		println!("Connected guilds:");
		for guild in ready.guilds {
			println!("{}", guild.id().0);
		}
	}
}

#[tokio::main]
async fn main() {
	let token = env::var(TOKEN_NAME).expect(
		format!(
			"Expected the discord token in the environment variable {}",
			TOKEN_NAME
		)
		.as_str(),
	);

	let mut client = Client::builder(&token)
		.event_handler(Handler)
		.await
		.expect("Error creating the client");

	if let Err(e) = client.start().await {
		eprintln!("Client error: {:?}", e);
	}
}
