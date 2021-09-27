mod config;
mod message;
mod music;

use crate::{
    config::{Config, CONFIG_FILE},
    music::{MUSIC_GROUP, MUSIC_HELP},
};

use serenity::{
    async_trait,
    client::{Client, Context, EventHandler},
    framework::StandardFramework,
    model::gateway::Ready,
};
use songbird::SerenityInit;
use tokio::signal::unix::{signal, SignalKind};

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
    let config = match Config::get_config() {
        Ok(c) => c,
        Err(e) => {
            println!("Error reading {}: {}", CONFIG_FILE, e);
            std::process::exit(1);
        }
    };

    let framework = StandardFramework::new()
        .configure(|c| c.prefix(&config.prefix))
        .help(&MUSIC_HELP)
        .group(&MUSIC_GROUP);

    let mut client = Client::builder(&config.token)
        .event_handler(Handler)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    // Register signal handlers
    {
        // SIGTERM
        let mut stream = signal(SignalKind::terminate()).expect("Error creating SIGTERM handler");
        let shard_manager = client.shard_manager.clone();
        tokio::spawn(async move {
            stream.recv().await;
            println!("Received SIGTERM, exiting");
            shard_manager.lock().await.shutdown_all().await;
        });

        // SIGINT
        let mut stream = signal(SignalKind::interrupt()).expect("Error creating SIGINT handler");
        let shard_manager = client.shard_manager.clone();
        tokio::spawn(async move {
            stream.recv().await;
            println!("Received SIGINT, exiting");
            shard_manager.lock().await.shutdown_all().await;
        });
    }

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));
}
