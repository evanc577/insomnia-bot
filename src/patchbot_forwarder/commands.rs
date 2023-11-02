use poise::serenity_prelude::Channel;

use crate::message::{SendMessage, SendableMessage};
use crate::patchbot_forwarder::helpers::{delete_from_table, guild_rules, insert_to_table};
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
    let rules = guild_rules(&ctx).await;
    let mut text = String::new();
    for rule in rules {
        text.push_str(&format!(
            r#"{}) "{}" in <#{}>  -> <#{}>"#,
            rule.id,
            rule.match_text,
            u64::from_str_radix(&rule.source_channel_id, 16).unwrap(),
            u64::from_str_radix(&rule.dest_channel_id, 16).unwrap()
        ));
        text.push('\n');
    }
    SendMessage::Normal(text).send_msg(ctx).await;
    Ok(())
}

/// Remove Patchbot forward rules
#[poise::command(
    slash_command,
    prefix_command,
    guild_only,
    required_permissions = "MANAGE_GUILD"
)]
pub async fn patchbot_remove(ctx: PoiseContext<'_>, id: i64) -> Result<(), PoiseError> {
    let deleted_rows = delete_from_table(&ctx, id).await;
    if deleted_rows != 0 {
        SendMessage::Normal("Deleted 1 rule").send_msg(ctx).await;
    } else {
        SendMessage::Error("No rules deleted").send_msg(ctx).await;
    }
    Ok(())
}
