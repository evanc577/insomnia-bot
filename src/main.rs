mod config;
mod link_embed;
mod message;
mod music;
mod package_update;
mod patchbot_forwarder;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;
use link_embed::reply_link_embeds;
use once_cell::sync::Lazy;
use package_update::update_packages;
use patchbot_forwarder::forward;
use poise::{serenity_prelude as serenity, Event, FrameworkContext};
use songbird::SerenityInit;
use tokio::signal::unix::{signal, SignalKind};

use crate::config::Config;
use crate::message::{SendMessage, SendableMessage};
use crate::music::{
    get_spotify_token_and_refresh, handle_voice_state_event, MusicError, QueueMutexMap,
};

pub type PoiseError = Box<dyn std::error::Error + Send + Sync>;
pub type PoiseContext<'a> = poise::Context<'a, Data, PoiseError>;
#[derive(Debug)]
pub struct Data {
    db_uri: String,

    // TODO: Use Spotify token for authenticated API calls
    #[allow(dead_code)]
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

/// Log command invocation
async fn pre_command(ctx: PoiseContext<'_>) {
    println!(
        "command {} called by {}#{:04}",
        ctx.command().qualified_name,
        ctx.author().name,
        ctx.author().discriminator
    );
}

async fn on_error(error: poise::FrameworkError<'_, Data, PoiseError>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            if let Some(e) = error.downcast_ref::<MusicError>() {
                // If command returned a MusicError, notify the caller by sending a reply
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

async fn on_event<U, E>(
    ctx: &serenity::Context,
    event: &Event<'_>,
    _framework: FrameworkContext<'_, U, E>,
    data: &Data,
) -> Result<(), PoiseError> {
    #[allow(clippy::single_match)]
    match event {
        Event::VoiceStateUpdate { new: state, .. } => {
            handle_voice_state_event(ctx, state).await;
        }
        Event::Message {
            new_message: message,
        } => {
            let http = ctx.http.clone();

            // Forward patchbot messages
            let db_uri = data.db_uri.as_str();
            if let Err(e) = forward(db_uri, http.clone(), message.clone()).await {
                eprintln!("Error forwarding patchbot message: {e}");
            }

            // Add embed preview for Tweets and Reddit links
            if let Err(e) = reply_link_embeds(http.clone(), message.clone()).await {
                eprintln!("Error sending link embed: {e}");
            }
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::get_config()?;
    let spotify_token =
        get_spotify_token_and_refresh(&config.spotify_client_id, &config.spotify_secret).await?;

    // Set up message forwarder
    let db_uri = patchbot_forwarder::create_table(&config).await;

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
        patchbot_forwarder::commands::patchbot_forward(),
        patchbot_forwarder::commands::patchbot_list(),
        patchbot_forwarder::commands::patchbot_remove(),
    ];

    // Configure Poise options
    let options = poise::FrameworkOptions {
        commands,
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(config.prefix),
            ..Default::default()
        },
        pre_command: |ctx| Box::pin(pre_command(ctx)),
        on_error: |error| Box::pin(on_error(error)),
        event_handler: |ctx, event, framework, data| {
            Box::pin(on_event(ctx, event, framework, data))
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
        .setup(move |_ctx, _ready, _framework| {
            Box::pin(async move {
                Ok(Data {
                    db_uri,
                    spotify_token,
                })
            })
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

    // Install packages and start auto updater
    update_packages().await?;

    let _ = client
        .start()
        .await
        .map_err(|why| println!("Client ended: {:?}", why));

    Ok(())
}
