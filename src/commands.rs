//! Slash commands for the runtime subscription flow, with state → county autocomplete.
//!
//!   /ebird-activate            key:<token>       (Manage Server) store/rotate the eBird key
//!   /ebird-fetch-subdivisions  state:<…>         (Manage Server) load + cache a state's counties
//!   /ebird-subscribe           state:<…> county:<…>  (Manage Server) alert THIS channel to a county
//!   /ebird-unsubscribe         county:<code>     (Manage Server) stop alerts for a county here
//!   /ebird-list                                  show this guild's subscriptions
//!   /ebird-status                                cadence + budget usage
//!   /ebird-onboarding          state:<…>         guided setup (handled in `onboarding`)
//!   /ebird-purge                                 DEBUG: delete bot-created channels (in `purge`)
//!
//! County autocomplete is cache-only for speed (Discord allows ~3s): counties are loaded
//! out-of-band by `/ebird-fetch-subdivisions`, then autocomplete serves the warm cache.
//! Replies are ephemeral.

use std::collections::HashSet;

use serenity::all::*;

use crate::counties::CountyCache;
use crate::ebird::EbirdClient;
use crate::poller::Schedule;
use crate::states;
use crate::store::Store;

pub fn commands() -> Vec<CreateCommand> {
    let admin = Permissions::MANAGE_GUILD;
    vec![
        CreateCommand::new("ebird-activate")
            .description("Activate (or rotate) this server's eBird API key")
            .default_member_permissions(admin)
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "key", "Your eBird API token")
                    .required(true),
            ),
        CreateCommand::new("ebird-subscribe")
            .description("Alert this channel to notable birds in a county")
            .default_member_permissions(admin)
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "state", "US state")
                    .required(true)
                    .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "county", "County in that state")
                    .required(true)
                    .set_autocomplete(true),
            ),
        CreateCommand::new("ebird-fetch-subdivisions")
            .description("Load a state's counties so they appear in /ebird-subscribe")
            .default_member_permissions(admin)
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "state", "US state")
                    .required(true)
                    .set_autocomplete(true),
            ),
        CreateCommand::new("ebird-unsubscribe")
            .description("Stop this channel's alerts for a county")
            .default_member_permissions(admin)
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "county",
                    "County region code, e.g. US-PA-091",
                )
                .required(true),
            ),
        CreateCommand::new("ebird-onboarding")
            .description("Guided setup: pick a state, choose counties, optionally create channels")
            .default_member_permissions(admin)
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "state", "US state")
                    .required(true)
                    .set_autocomplete(true),
            )
            .add_option(
                CreateCommandOption::new(
                    CommandOptionType::String,
                    "suffix",
                    "Channel name style: <county>-<suffix> (optional, e.g. rba)",
                )
                .required(false),
            ),
        CreateCommand::new("ebird-list").description("List this server's eBird subscriptions"),
        CreateCommand::new("ebird-status")
            .description("Show subscriptions, polling cadence, and request-budget usage"),
        CreateCommand::new("ebird-purge")
            .description("DEBUG: delete all bot-created county channels + eBird Alerts categories")
            .default_member_permissions(admin),
    ]
}

/// Handle a submitted command.
pub async fn dispatch(
    ctx: &Context,
    cmd: &CommandInteraction,
    store: &Store,
    counties: &CountyCache,
) -> anyhow::Result<()> {
    let Some(guild_id) = cmd.guild_id.map(|g| g.get().to_string()) else {
        return reply(ctx, cmd, "Use this in a server.").await;
    };

    match cmd.data.name.as_str() {
        "ebird-activate" => {
            let key = opt_str(cmd, "key").to_string();
            // Validate against eBird before storing, so a typo'd key fails loudly here
            // rather than silently at poll time. Deferred because it's a network call.
            cmd.create_response(
                &ctx.http,
                CreateInteractionResponse::Defer(
                    CreateInteractionResponseMessage::new().ephemeral(true),
                ),
            )
            .await?;
            let valid = EbirdClient::new(key.clone())
                .recent_notable("US-PA-091", 1)
                .await
                .is_ok();
            let msg = if valid {
                store.activate(&guild_id, &key).await?;
                "✅ eBird key activated for this server."
            } else {
                "❌ eBird rejected that key. Double-check your token at \
                 https://ebird.org/api/keygen and try again."
            };
            cmd.edit_response(&ctx.http, EditInteractionResponse::new().content(msg))
                .await?;
            Ok(())
        }
        "ebird-fetch-subdivisions" => {
            let state = opt_str(cmd, "state");
            if state.is_empty() {
                return reply(ctx, cmd, "Pick a state.").await;
            }
            let Some(token) = store.token_for(&guild_id).await? else {
                return reply(ctx, cmd, "Run `/ebird-activate` first.").await;
            };
            let client = EbirdClient::new(token);
            match counties.get_or_load(state, &client).await {
                Ok(list) => {
                    reply(
                        ctx,
                        cmd,
                        &format!(
                            "✅ Loaded {} counties for {state}. You can now `/ebird-subscribe`.",
                            list.len()
                        ),
                    )
                    .await
                }
                Err(e) => reply(ctx, cmd, &format!("Failed to load counties for {state}: {e}")).await,
            }
        }
        "ebird-subscribe" => {
            if !store.has_key(&guild_id).await {
                return reply(ctx, cmd, "Run `/ebird-activate` first.").await;
            }
            let state = opt_str(cmd, "state");
            let county_code = opt_str(cmd, "county");
            if county_code.is_empty() || county_code == COUNTY_HINT {
                return reply(
                    ctx,
                    cmd,
                    "Load this state's counties with `/ebird-fetch-subdivisions` first, then pick a county.",
                )
                .await;
            }
            // The cache is warm from autocomplete; fall back to the code if not.
            let name = counties
                .name_for(state, county_code)
                .await
                .unwrap_or_else(|| county_code.to_string());
            let added = store
                // managed = false: subscribing an existing channel, not one we created.
                .subscribe(&guild_id, cmd.channel_id.get(), county_code, &name, false)
                .await?;
            let msg = if added {
                format!("✅ This channel will get notable-bird alerts for {name} ({county_code}).")
            } else {
                format!("This channel is already subscribed to {name}.")
            };
            reply(ctx, cmd, &msg).await
        }
        "ebird-unsubscribe" => {
            let county = opt_str(cmd, "county");
            let removed = store
                .unsubscribe(&guild_id, cmd.channel_id.get(), county)
                .await?;
            let msg = if removed {
                format!("Removed this channel's subscription to {county}.")
            } else {
                format!("This channel wasn't subscribed to {county}.")
            };
            reply(ctx, cmd, &msg).await
        }
        "ebird-list" => {
            let subs = store.list(&guild_id).await;
            let msg = if subs.is_empty() {
                "No subscriptions yet. Use `/ebird-subscribe`.".to_string()
            } else {
                let mut lines = String::from("**Subscriptions:**\n");
                for s in subs {
                    lines.push_str(&format!(
                        "• <#{}> → {} ({})\n",
                        s.channel_id, s.name, s.region_code
                    ));
                }
                lines
            };
            reply(ctx, cmd, &msg).await
        }
        other => reply(ctx, cmd, &format!("Unknown command: {other}")).await,
    }
}

/// `/ebird-status` — surface the effective cadence + budget usage derived from this
/// guild's distinct-county count.
pub async fn status(
    ctx: &Context,
    cmd: &CommandInteraction,
    store: &Store,
    schedule: &Schedule,
) -> anyhow::Result<()> {
    let Some(guild_id) = cmd.guild_id.map(|g| g.get().to_string()) else {
        return reply(ctx, cmd, "Use this in a server.").await;
    };
    let subs = store.list(&guild_id).await;
    let counties: HashSet<&str> = subs.iter().map(|s| s.region_code.as_str()).collect();
    let plan = schedule.plan(counties.len());
    let key = if store.has_key(&guild_id).await {
        "✅ activated"
    } else {
        "❌ not activated — run `/ebird-activate`"
    };
    let mode = if plan.active_now {
        "active (day)"
    } else {
        "night backoff"
    };
    let msg = format!(
        "**eBird status**\n\
         • Key: {key}\n\
         • Subscriptions: {} across {} distinct counties\n\
         • Cadence: {} min day / {} min night — currently **{mode}**\n\
         • Est. requests/day: ~{} of {} budget",
        subs.len(),
        counties.len(),
        plan.day_min,
        plan.night_min,
        plan.est_daily_requests,
        plan.budget,
    );
    reply(ctx, cmd, &msg).await
}

/// Handle an autocomplete interaction for `state` / `county`. Cache-only — never makes a
/// network call, so it always responds within Discord's ~3s window. Counties are loaded
/// out-of-band by `/ebird-fetch-subdivisions`.
pub async fn autocomplete(
    ctx: &Context,
    cmd: &CommandInteraction,
    counties: &CountyCache,
) -> anyhow::Result<()> {
    let Some((focused_name, partial)) = focused_option(cmd) else {
        return Ok(());
    };

    let choices: Vec<(String, String)> = match focused_name {
        "state" => states::filter(partial),
        "county" => {
            let state = opt_str(cmd, "state");
            if state.is_empty() {
                Vec::new()
            } else if let Some(list) = counties.get_cached(state).await {
                filter_counties(&list, partial)
            } else {
                vec![(
                    "⚠ Run /ebird-fetch-subdivisions for this state first".to_string(),
                    COUNTY_HINT.to_string(),
                )]
            }
        }
        _ => Vec::new(),
    };

    let mut response = CreateAutocompleteResponse::new();
    for (name, value) in choices.into_iter().take(25) {
        response = response.add_string_choice(name, value);
    }
    cmd.create_response(&ctx.http, CreateInteractionResponse::Autocomplete(response))
        .await?;
    Ok(())
}

/// Value of the hint choice shown when a state's counties aren't loaded yet.
const COUNTY_HINT: &str = "__load_counties_first__";

/// County autocomplete choices `(display_name, county_code)` matching `partial`.
fn filter_counties(list: &[(String, String)], partial: &str) -> Vec<(String, String)> {
    let p = partial.to_lowercase();
    list.iter()
        .filter(|(_code, name)| p.is_empty() || name.to_lowercase().contains(&p))
        .take(25)
        .map(|(code, name)| (name.clone(), code.clone()))
        .collect()
}

/// Value of a command option by name (handles both filled and autocomplete-in-progress).
fn opt_str<'a>(cmd: &'a CommandInteraction, name: &str) -> &'a str {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .map(|o| match &o.value {
            CommandDataOptionValue::String(s) => s.as_str(),
            CommandDataOptionValue::Autocomplete { value, .. } => value.as_str(),
            _ => "",
        })
        .unwrap_or("")
}

/// The currently-focused autocomplete option as `(name, partial_value)`.
fn focused_option(cmd: &CommandInteraction) -> Option<(&str, &str)> {
    cmd.data.options.iter().find_map(|o| match &o.value {
        CommandDataOptionValue::Autocomplete { value, .. } => Some((o.name.as_str(), value.as_str())),
        _ => None,
    })
}

/// Send an ephemeral reply to the command.
async fn reply(ctx: &Context, cmd: &CommandInteraction, content: &str) -> anyhow::Result<()> {
    cmd.create_response(
        &ctx.http,
        CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(content)
                .ephemeral(true),
        ),
    )
    .await?;
    Ok(())
}
