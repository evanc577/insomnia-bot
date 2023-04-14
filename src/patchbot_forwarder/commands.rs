use crate::{
    message::{SendMessage, SendableMessage},
    patchbot_forwarder::helpers::insert_to_table,
};
use poise::serenity_prelude::Channel;

use crate::{PoiseContext, PoiseError};

/// Add rule to forward Patchbot messages to a different channel
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn patchbot_forward(
    ctx: PoiseContext<'_>,
    match_text: String,
    source_channel: Channel,
    dest_channel: Channel,
) -> Result<(), PoiseError> {
    insert_to_table(
        &ctx,
        &match_text,
        *source_channel.id().as_u64(),
        *dest_channel.id().as_u64(),
    )
    .await;

    SendMessage::Normal(format!(
        r#"Now forwarding "{}" messages from {} to {}"#,
        match_text, source_channel, dest_channel,
    ))
    .send_msg(ctx)
    .await;

    Ok(())
}

/// List Patchbot forward rules
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn patchbot_list(ctx: PoiseContext<'_>) -> Result<(), PoiseError> {
    SendMessage::Normal("Unimplemented, contact bot author")
        .send_msg(ctx)
        .await;
    Ok(())
}

/// Remove Patchbot forward rules
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn patchbot_remove(ctx: PoiseContext<'_>) -> Result<(), PoiseError> {
    SendMessage::Normal("Unimplemented, contact bot author")
        .send_msg(ctx)
        .await;
    Ok(())
}
