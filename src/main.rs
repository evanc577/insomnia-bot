mod config;
mod msg;
mod music;

use crate::{
    config::{Config, CONFIG_FILE},
    music::{MUSIC_GROUP, MY_HELP},
};

use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::StandardFramework,
    model::gateway::Ready,
};
use songbird::SerenityInit;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    // Configure the client with your Discord bot token in the environment.
    let config = match Config::read_config() {
        Ok(c) => c,
        Err(e) => {
            println!("Error reading {}: {}", CONFIG_FILE, e);
            std::process::exit(1);
        }
    };

    let framework = StandardFramework::new()
        .configure(|c| c.prefix(&config.prefix))
        .help(&MY_HELP)
        .group(&MUSIC_GROUP);

    let mut client = Client::builder(&config.token)
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
