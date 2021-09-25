mod msg;
mod music;

use crate::music::{MUSIC_GROUP, MY_HELP};

use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::StandardFramework,
    model::gateway::Ready,
};
use songbird::SerenityInit;
use std::env;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c.prefix("~"))
        .help(&MY_HELP)
        .group(&MUSIC_GROUP);

    let mut client = Client::builder(&token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}

