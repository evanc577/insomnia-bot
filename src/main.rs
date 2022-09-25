mod config;
mod message;
mod music;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use once_cell::sync::Lazy;
use poise::serenity_prelude as serenity;
use songbird::SerenityInit;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::Config;
use crate::message::{SendMessage, SendableMessage};
use crate::music::error::MusicError;
use crate::music::queue::QueueMutexMap;
use crate::music::spotify::auth::get_token_and_refresh;

pub type PoiseError = Box<dyn std::error::Error + Send + Sync>;
pub type PoiseContext<'a> = poise::Context<'a, Data, PoiseError>;
#[derive(Debug)]
pub struct Data {
    spotify_token: Arc<Mutex<String>>,
}

pub static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .use_rustls_tls()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
});

/// Registers slash commands in this guild or globally
#[poise::command(prefix_command, hide_in_help, owners_only)]
async fn register(ctx: PoiseContext<'_>) -> Result<(), PoiseError> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    println!("Registering...");
    Ok(())
}

/// Show this help menu
#[poise::command(slash_command, prefix_command, track_edits)]
async fn help(
    ctx: PoiseContext<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), PoiseError> {
    poise::builtins::help(ctx, command.as_deref(), Default::default()).await?;
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, PoiseError>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            if let Some(e) = error.downcast_ref::<MusicError>() {
                match e {
                    MusicError::Internal(e) => {
                        eprintln!("Internal error: {:?}", e);
                        SendMessage::Error("an internal error occured")
                            .send_msg(ctx)
                            .await;
                    }
                    _ => {
                        SendMessage::Error(e.to_string()).send_msg(ctx).await;
                    }
                }
            }
        }
        poise::FrameworkError::ArgumentParse { error, ctx, .. } => {
            SendMessage::Error(error.to_string()).send_msg(ctx).await;
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                eprintln!("Error while handling error: {}", e)
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::get_config()?;
    let spotify_token =
        get_token_and_refresh(&config.spotify_client_id, &config.spotify_secret).await?;

    // Add bot commands
    let commands = vec![
        register(),
        help(),
        music::commands::album(),
        music::commands::list(),
        music::commands::pause(),
        music::commands::play(),
        music::commands::remove(),
        music::commands::skip(),
        music::commands::song(),
        music::commands::stop(),
        music::commands::video(),
    ];

    // Configure Poise options
    let options = poise::FrameworkOptions {
        commands,
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(config.prefix),
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        pre_command: |ctx| {
            Box::pin(async move {
                println!(
                    "command {} called by {}#{:04}",
                    ctx.command().qualified_name,
                    ctx.author().name,
                    ctx.author().discriminator
                );
            })
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .options(options)
        .client_settings(|cb| cb.register_songbird())
        .token(config.discord_token)
        .intents(
            serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT,
        )
        .user_data_setup(move |_ctx, _ready, _framework| {
            Box::pin(async move { Ok(Data { spotify_token }) })
        })
        .build()
        .await
        .unwrap();

    let mut client = framework.client();
    {
        let mut data = client.data.write().await;
        data.insert::<QueueMutexMap>(HashMap::new());
    }

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

    Ok(())
}
