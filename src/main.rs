use std::env;

use poise::{
	async_trait,
	serenity::{
		client::{parse_token, Context, EventHandler},
		model::{channel::Message, gateway::Ready},
		Client,
	},
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

	async fn ready(&self, ctx: Context, ready: Ready) {
		println!("{} is connected!", ready.user.name);
		if ready.guilds.is_empty() {
			println!("No connected guilds.");
			return;
		}
		println!("Connected guilds:");
		for guild in ready.guilds {
			let guild_data = guild
				.id()
				.to_partial_guild(&ctx.http)
				.await
				.expect(format!("Unable to get guild with id {}", guild.id()).as_str());
			println!(
				"{} - {} ({})",
				guild.id().0,
				guild_data.name,
				guild_data.approximate_member_count.unwrap_or(0)
			);
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
	let app_id = parse_token(&token).expect("Token is invalid").bot_user_id;

	let mut client = Client::builder(&token)
		.application_id(app_id.0)
		.event_handler(Handler)
		.await
		.expect("Error creating the client");

	if let Err(e) = client.start().await {
		eprintln!("Client error: {:?}", e);
	}
}
