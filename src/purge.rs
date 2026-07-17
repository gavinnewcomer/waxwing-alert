//! DEBUG: `/ebird-purge` deletes the channels and categories the bot **created** (recorded
//! by id at creation time), to make offboarding / re-testing easy.
//!
//! Driving this off recorded ids — not the current channel layout — means it still works
//! after a server renames the categories or moves channels into different sections. Channels
//! a server pointed us at via *Use this channel* are `managed = false` and never touched.
//! Guarded by a confirmation button.

use serenity::all::*;
use tracing::warn;

use crate::store::Store;

/// `/ebird-purge` — confirm deleting the bot-created channels + categories.
pub async fn start(ctx: &Context, cmd: &CommandInteraction, store: &Store) -> anyhow::Result<()> {
    let Some(guild) = cmd.guild_id else {
        return respond(ctx, cmd, "Use this in a server.").await;
    };
    let guild_id = guild.get().to_string();

    cmd.create_response(
        &ctx.http,
        CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new().ephemeral(true)),
    )
    .await?;

    let channels = store.managed_channel_ids(&guild_id).await;
    let cats = store.managed_category_ids(&guild_id).await;
    if channels.is_empty() && cats.is_empty() {
        cmd.edit_response(
            &ctx.http,
            EditInteractionResponse::new()
                .content("Nothing to purge — no bot-created channels recorded for this server."),
        )
        .await?;
        return Ok(());
    }

    let content = format!(
        "⚠️ **Debug purge** will delete **{} bot-created channels** and **{} categories**. \
         This cannot be undone. (Channels you subscribed via *Use this channel* are left alone.)",
        channels.len(),
        cats.len()
    );
    let buttons = vec![
        CreateButton::new("purge:confirm")
            .label(format!("Delete {} channels", channels.len()))
            .style(ButtonStyle::Danger),
        CreateButton::new("purge:cancel")
            .label("Cancel")
            .style(ButtonStyle::Secondary),
    ];
    cmd.edit_response(
        &ctx.http,
        EditInteractionResponse::new()
            .content(content)
            .components(vec![CreateActionRow::Buttons(buttons)]),
    )
    .await?;
    Ok(())
}

/// Handle the confirm / cancel buttons. Deleting a channel fires `channel_delete`, which
/// prunes its subscription (and drops a category from the managed list) automatically.
pub async fn handle_component(
    ctx: &Context,
    component: &ComponentInteraction,
    store: &Store,
) -> anyhow::Result<()> {
    match component.data.custom_id.as_str() {
        "purge:cancel" => {
            component
                .create_response(
                    &ctx.http,
                    CreateInteractionResponse::UpdateMessage(
                        CreateInteractionResponseMessage::new()
                            .content("Purge cancelled.")
                            .components(vec![]),
                    ),
                )
                .await?;
        }
        "purge:confirm" => {
            let Some(guild) = component.guild_id else {
                return Ok(());
            };
            let guild_id = guild.get().to_string();
            // Deleting many channels is slow — defer the update.
            component
                .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
                .await?;

            let channels = store.managed_channel_ids(&guild_id).await;
            let cats = store.managed_category_ids(&guild_id).await;

            let mut deleted = 0u32;
            // Channels first, then their (now-empty) categories.
            for id in channels.iter().chain(cats.iter()) {
                match ChannelId::new(*id).delete(&ctx.http).await {
                    Ok(_) => deleted += 1,
                    Err(e) => warn!(channel = id, "purge: delete failed: {e}"),
                }
                // Deletion is rate-limited too; pace ourselves.
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            component
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new()
                        .content(format!("🗑️ Purged {deleted} channels/categories."))
                        .components(vec![]),
                )
                .await?;
        }
        _ => {}
    }
    Ok(())
}

async fn respond(ctx: &Context, cmd: &CommandInteraction, content: &str) -> anyhow::Result<()> {
    cmd.create_response(
        &ctx.http,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .ephemeral(true)
                .content(content),
        ),
    )
    .await?;
    Ok(())
}
