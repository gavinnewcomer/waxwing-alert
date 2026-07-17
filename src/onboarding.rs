//! Guided onboarding: `/ebird-onboarding state:<…>` → paginated multi-select of counties
//! → choose whether to create a channel per county or use the current channel.
//!
//! Uses Discord message components (select menu + buttons). Because a state can have far
//! more than the 25 options a select menu allows, counties are paginated and the running
//! selection is held in an in-memory session keyed by (guild, user).

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use serenity::all::*;
use tokio::sync::RwLock;
use tracing::warn;

use crate::counties::CountyCache;
use crate::ebird::EbirdClient;
use crate::store::Store;

const PAGE_SIZE: usize = 25;

/// One user's in-progress onboarding.
struct OnboardState {
    state_code: String,
    /// Optional channel-name suffix: channels are named `<county>-<suffix>` (empty = just
    /// the county name).
    suffix: String,
    /// (county_code, county_name), full list for the state.
    counties: Arc<Vec<(String, String)>>,
    page: usize,
    /// Selected county codes (accumulated across pages).
    selected: HashSet<String>,
}

#[derive(Clone, Default)]
pub struct OnboardSessions {
    inner: Arc<RwLock<HashMap<(String, String), OnboardState>>>,
}

impl OnboardSessions {
    pub fn new() -> Self {
        Self::default()
    }

    async fn set(&self, key: (String, String), st: OnboardState) {
        self.inner.write().await.insert(key, st);
    }

    async fn remove(&self, key: &(String, String)) {
        self.inner.write().await.remove(key);
    }

    /// Merge this page's select-menu values into the running selection.
    async fn record_page_selection(&self, key: &(String, String), values: Vec<String>) {
        let mut map = self.inner.write().await;
        if let Some(st) = map.get_mut(key) {
            let start = st.page * PAGE_SIZE;
            let end = (start + PAGE_SIZE).min(st.counties.len());
            let page_codes: HashSet<&str> =
                st.counties[start..end].iter().map(|(c, _)| c.as_str()).collect();
            st.selected.retain(|c| !page_codes.contains(c.as_str()));
            st.selected.extend(values);
        }
    }

    async fn change_page(&self, key: &(String, String), forward: bool) {
        let mut map = self.inner.write().await;
        if let Some(st) = map.get_mut(key) {
            let pages = pages_of(st.counties.len());
            if forward && st.page + 1 < pages {
                st.page += 1;
            } else if !forward && st.page > 0 {
                st.page -= 1;
            }
        }
    }

    /// Select every county in the state (all pages).
    async fn select_all(&self, key: &(String, String)) {
        let mut map = self.inner.write().await;
        if let Some(st) = map.get_mut(key) {
            st.selected = st.counties.iter().map(|(c, _)| c.clone()).collect();
        }
    }

    /// Clear the whole selection.
    async fn clear_all(&self, key: &(String, String)) {
        let mut map = self.inner.write().await;
        if let Some(st) = map.get_mut(key) {
            st.selected.clear();
        }
    }

    /// Render the current county-selection page (content + components).
    async fn page_view(&self, key: &(String, String)) -> Option<(String, Vec<CreateActionRow>)> {
        let map = self.inner.read().await;
        map.get(key).map(page_view)
    }

    /// Render the "how to set up" step (content + buttons).
    async fn choice_view(&self, key: &(String, String)) -> Option<(String, Vec<CreateActionRow>)> {
        let map = self.inner.read().await;
        map.get(key).map(choice_view)
    }

    /// Remove the session and return (channel-name suffix, selected `(code, name)`).
    async fn take(&self, key: &(String, String)) -> Option<(String, Vec<(String, String)>)> {
        let mut map = self.inner.write().await;
        let st = map.remove(key)?;
        let selected = st
            .counties
            .iter()
            .filter(|(c, _)| st.selected.contains(c))
            .cloned()
            .collect();
        Some((st.suffix, selected))
    }
}

/// Handle `/ebird-onboarding`: load the state's counties and show page 0.
pub async fn start(
    ctx: &Context,
    cmd: &CommandInteraction,
    store: &Store,
    counties: &CountyCache,
    sessions: &OnboardSessions,
) -> anyhow::Result<()> {
    let Some(guild_id) = cmd.guild_id.map(|g| g.get().to_string()) else {
        return ephemeral(ctx, cmd, "Use this in a server.").await;
    };
    let user_id = cmd.user.id.get().to_string();
    let state = option_str(cmd, "state").to_string();
    let suffix = option_str(cmd, "suffix").to_string();

    // Fetching counties can exceed Discord's 3s window, so defer first.
    cmd.create_response(
        &ctx.http,
        CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new().ephemeral(true)),
    )
    .await?;

    let Some(token) = store.token_for(&guild_id).await? else {
        cmd.edit_response(&ctx.http, EditInteractionResponse::new().content("Run `/ebird-activate` first."))
            .await?;
        return Ok(());
    };
    if state.is_empty() {
        cmd.edit_response(&ctx.http, EditInteractionResponse::new().content("Pick a state."))
            .await?;
        return Ok(());
    }

    let client = EbirdClient::new(token);
    let list = match counties.get_or_load(&state, &client).await {
        Ok(l) if !l.is_empty() => l,
        _ => {
            cmd.edit_response(
                &ctx.http,
                EditInteractionResponse::new().content(format!("Couldn't load counties for {state}.")),
            )
            .await?;
            return Ok(());
        }
    };

    let key = (guild_id, user_id);
    sessions
        .set(
            key.clone(),
            OnboardState {
                state_code: state,
                suffix,
                counties: list,
                page: 0,
                selected: HashSet::new(),
            },
        )
        .await;

    if let Some((content, rows)) = sessions.page_view(&key).await {
        cmd.edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(content).components(rows),
        )
        .await?;
    }
    Ok(())
}

/// Handle a component interaction whose custom_id starts with `onb:`.
pub async fn handle_component(
    ctx: &Context,
    component: &ComponentInteraction,
    store: &Store,
    sessions: &OnboardSessions,
) -> anyhow::Result<()> {
    let id = component.data.custom_id.clone();
    if !id.starts_with("onb:") {
        return Ok(());
    }
    let Some(guild_id) = component.guild_id.map(|g| g.get().to_string()) else {
        return Ok(());
    };
    let key = (guild_id.clone(), component.user.id.get().to_string());

    match id.as_str() {
        "onb:sel" => {
            let values = match &component.data.kind {
                ComponentInteractionDataKind::StringSelect { values } => values.clone(),
                _ => Vec::new(),
            };
            sessions.record_page_selection(&key, values).await;
            // Just acknowledge — do NOT re-render. Re-rendering the components while the
            // user is interacting with the select menu is what caused button clicks to
            // need a second press (they'd land on components we'd just replaced).
            component
                .create_response(&ctx.http, CreateInteractionResponse::Acknowledge)
                .await?;
        }
        "onb:prev" => {
            sessions.change_page(&key, false).await;
            update_from_page(ctx, component, sessions, &key).await?;
        }
        "onb:next" => {
            sessions.change_page(&key, true).await;
            update_from_page(ctx, component, sessions, &key).await?;
        }
        "onb:all" => {
            sessions.select_all(&key).await;
            update_from_page(ctx, component, sessions, &key).await?;
        }
        "onb:clear" => {
            sessions.clear_all(&key).await;
            update_from_page(ctx, component, sessions, &key).await?;
        }
        "onb:done" => {
            if let Some((content, rows)) = sessions.choice_view(&key).await {
                update_message(ctx, component, content, rows).await?;
            }
        }
        "onb:here" => finalize_here(ctx, component, store, sessions, &key).await?,
        "onb:chan" => finalize_channels(ctx, component, store, sessions, &guild_id, &key).await?,
        "onb:cancel" => {
            sessions.remove(&key).await;
            update_message(ctx, component, "Onboarding cancelled.".into(), vec![]).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn update_from_page(
    ctx: &Context,
    component: &ComponentInteraction,
    sessions: &OnboardSessions,
    key: &(String, String),
) -> anyhow::Result<()> {
    if let Some((content, rows)) = sessions.page_view(key).await {
        update_message(ctx, component, content, rows).await?;
    }
    Ok(())
}

async fn finalize_here(
    ctx: &Context,
    component: &ComponentInteraction,
    store: &Store,
    sessions: &OnboardSessions,
    key: &(String, String),
) -> anyhow::Result<()> {
    component.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await?;
    let channel_id = component.channel_id.get();
    let msg = match sessions.take(key).await {
        Some((_, selected)) if !selected.is_empty() => {
            let mut n = 0;
            for (code, name) in &selected {
                // managed = false: this is the server's own channel, never purge it.
                if store.subscribe(&key.0, channel_id, code, name, false).await? {
                    n += 1;
                }
            }
            format!("✅ Subscribed <#{channel_id}> to {n} counties.")
        }
        _ => "No counties selected (or the session expired).".to_string(),
    };
    component
        .edit_response(&ctx.http, EditInteractionResponse::new().content(msg).components(vec![]))
        .await?;
    Ok(())
}

async fn finalize_channels(
    ctx: &Context,
    component: &ComponentInteraction,
    store: &Store,
    sessions: &OnboardSessions,
    guild_id: &str,
    key: &(String, String),
) -> anyhow::Result<()> {
    // Creating N channels is N API calls — defer so we don't miss the 3s window.
    component.create_response(&ctx.http, CreateInteractionResponse::Acknowledge).await?;
    let Some(guild) = component.guild_id else {
        return Ok(());
    };

    let msg = match sessions.take(key).await {
        Some((suffix, selected)) if !selected.is_empty() => {
            // Nest channels under "eBird Alerts" categories, spilling into numbered
            // categories so none exceeds Discord's 50-channels-per-category limit.
            let mut cats = CategoryAllocator::load(ctx, guild).await;

            // Read-only alert channels: deny @everyone Send Messages, allow the bot.
            let readonly = readonly_overwrites(ctx, guild);

            let mut created = 0u32;
            let mut errored = false;
            for (code, name) in &selected {
                let category = cats.next(ctx, guild).await;
                let mut builder = CreateChannel::new(channel_name(name, &suffix))
                    .kind(ChannelType::Text)
                    .permissions(readonly.clone());
                if let Some(cat) = category {
                    builder = builder.category(cat);
                }
                match guild.create_channel(&ctx.http, builder).await {
                    Ok(ch) => {
                        // managed = true: the bot created this channel, so purge may delete it.
                        store.subscribe(guild_id, ch.id.get(), code, name, true).await?;
                        created += 1;
                    }
                    Err(e) => {
                        // If we can't create one channel we can't create any — stop here
                        // instead of failing identically for every remaining county.
                        warn!("create_channel failed for {name}: {e}");
                        errored = true;
                        break;
                    }
                }
                // Discord heavily rate-limits channel creation; pace ourselves to avoid a
                // burst of 429s (serenity still waits on any that slip through).
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }

            // Record the categories we created so purge can remove them by id later.
            for cat in &cats.created {
                let _ = store.record_category(guild_id, cat.get()).await;
            }

            match (created, errored) {
                (n, false) => format!("✅ Created {n} channels and subscribed them."),
                (0, true) => "❌ I couldn't create channels — I need **Manage Channels** and \
                     **Manage Roles** (Manage Roles is required to make the channels read-only). \
                     Grant my role those (Server Settings → Roles), or re-run and pick \
                     *Use this channel*."
                    .to_string(),
                (n, true) => format!(
                    "Created {n} channels, then stopped on an error (permission or rate limit). \
                     Fix it and re-run for the rest, or use *Use this channel*."
                ),
            }
        }
        _ => "No counties selected (or the session expired).".to_string(),
    };
    component
        .edit_response(&ctx.http, EditInteractionResponse::new().content(msg).components(vec![]))
        .await?;
    Ok(())
}

/// Overwrites that make a channel read-only: `@everyone` can view but not send; the bot
/// can send. Setting these requires the bot to have Manage Roles.
fn readonly_overwrites(ctx: &Context, guild: GuildId) -> Vec<PermissionOverwrite> {
    let bot_id = ctx.cache.current_user().id;
    vec![
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::SEND_MESSAGES
                | Permissions::SEND_MESSAGES_IN_THREADS
                | Permissions::CREATE_PUBLIC_THREADS
                | Permissions::CREATE_PRIVATE_THREADS,
            // The @everyone role's id equals the guild id.
            kind: PermissionOverwriteType::Role(RoleId::new(guild.get())),
        },
        PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(bot_id),
        },
    ]
}

/// Doles out "eBird Alerts" category channels for county channels to nest under, spilling
/// into numbered categories ("eBird Alerts 2", …) so none exceeds Discord's 50-channel cap.
/// Existing categories are filled first; new ones are created only as needed.
struct CategoryAllocator {
    /// (category_id, remaining_capacity), only those with room, in fill order.
    with_space: VecDeque<(ChannelId, usize)>,
    /// Next suffix number to use when creating a category.
    next_num: u32,
    /// Categories this allocator created (to record as bot-managed).
    created: Vec<ChannelId>,
}

impl CategoryAllocator {
    const CAP: usize = 50;

    async fn load(ctx: &Context, guild: GuildId) -> Self {
        let existing = guild.channels(&ctx.http).await.unwrap_or_default();

        // Current occupancy of every category (all child channels count toward the cap).
        let mut occupancy: HashMap<ChannelId, usize> = HashMap::new();
        for ch in existing.values() {
            if let Some(parent) = ch.parent_id {
                *occupancy.entry(parent).or_default() += 1;
            }
        }

        // Our "eBird Alerts[ N]" categories, in numeric order.
        let mut cats: Vec<(u32, ChannelId)> = existing
            .values()
            .filter(|c| c.kind == ChannelType::Category)
            .filter_map(|c| category_number(&c.name).map(|n| (n, c.id)))
            .collect();
        cats.sort_by_key(|(n, _)| *n);

        let with_space = cats
            .iter()
            .map(|(_, id)| (*id, Self::CAP.saturating_sub(*occupancy.get(id).unwrap_or(&0))))
            .filter(|(_, rem)| *rem > 0)
            .collect();
        let next_num = cats.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;

        Self {
            with_space,
            next_num,
            created: Vec::new(),
        }
    }

    /// A category with room for one more channel, creating a new numbered one if all are
    /// full. `None` only if a needed category couldn't be created (channels go to root).
    async fn next(&mut self, ctx: &Context, guild: GuildId) -> Option<ChannelId> {
        while let Some(front) = self.with_space.front_mut() {
            if front.1 > 0 {
                front.1 -= 1;
                return Some(front.0);
            }
            self.with_space.pop_front();
        }
        let name = if self.next_num <= 1 {
            "eBird Alerts".to_string()
        } else {
            format!("eBird Alerts {}", self.next_num)
        };
        self.next_num += 1;
        match guild
            .create_channel(&ctx.http, CreateChannel::new(name).kind(ChannelType::Category))
            .await
        {
            Ok(ch) => {
                self.created.push(ch.id);
                self.with_space.push_back((ch.id, Self::CAP - 1));
                Some(ch.id)
            }
            Err(_) => None,
        }
    }
}

/// Parse an "eBird Alerts" category name to its number (base = 1, "eBird Alerts 2" = 2).
/// `Some` iff the name is one of our managed categories.
pub fn category_number(name: &str) -> Option<u32> {
    let lower = name.to_lowercase();
    let base = "ebird alerts";
    if lower == base {
        return Some(1);
    }
    lower.strip_prefix(base)?.trim().parse::<u32>().ok()
}

// --- rendering (pure) --------------------------------------------------------

fn pages_of(total: usize) -> usize {
    total.div_ceil(PAGE_SIZE).max(1)
}

fn page_view(st: &OnboardState) -> (String, Vec<CreateActionRow>) {
    let total = st.counties.len();
    let pages = pages_of(total);
    let start = st.page * PAGE_SIZE;
    let end = (start + PAGE_SIZE).min(total);
    let slice = &st.counties[start..end];

    let options: Vec<CreateSelectMenuOption> = slice
        .iter()
        .map(|(code, name)| {
            CreateSelectMenuOption::new(truncate(name, 100), code.clone())
                .default_selection(st.selected.contains(code))
        })
        .collect();

    let menu = CreateSelectMenu::new("onb:sel", CreateSelectMenuKind::String { options })
        .min_values(0)
        .max_values(slice.len().max(1) as u8)
        .placeholder("Select counties on this page");

    let nav = vec![
        CreateButton::new("onb:prev")
            .label("◀ Prev")
            .style(ButtonStyle::Secondary)
            .disabled(st.page == 0),
        CreateButton::new("onb:page")
            .label(format!("Page {}/{}", st.page + 1, pages))
            .style(ButtonStyle::Secondary)
            .disabled(true),
        CreateButton::new("onb:next")
            .label("Next ▶")
            .style(ButtonStyle::Secondary)
            .disabled(st.page + 1 >= pages),
        CreateButton::new("onb:done")
            .label(format!("Done ({} selected)", st.selected.len()))
            .style(ButtonStyle::Success),
    ];

    let bulk = vec![
        CreateButton::new("onb:all")
            .label(format!("Select all {total}"))
            .style(ButtonStyle::Secondary)
            .disabled(st.selected.len() == total),
        CreateButton::new("onb:clear")
            .label("Clear")
            .style(ButtonStyle::Secondary)
            .disabled(st.selected.is_empty()),
    ];

    let content = format!(
        "**Onboarding — {}**\nSelect counties (or **Select all**), page through if needed, then **Done**.",
        state_name(&st.state_code)
    );
    (
        content,
        vec![
            CreateActionRow::SelectMenu(menu),
            CreateActionRow::Buttons(nav),
            CreateActionRow::Buttons(bulk),
        ],
    )
}

fn choice_view(st: &OnboardState) -> (String, Vec<CreateActionRow>) {
    let names: Vec<&str> = st
        .counties
        .iter()
        .filter(|(c, _)| st.selected.contains(c))
        .map(|(_, n)| n.as_str())
        .collect();
    let list = if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    };
    let example = names
        .first()
        .map(|&n| channel_name(n, &st.suffix))
        .unwrap_or_default();
    let content = format!(
        "**{} counties selected:** {}\n\nHow should I set them up? \
         (channels named like `#{}`)",
        st.selected.len(),
        truncate(&list, 1400),
        example
    );
    let buttons = vec![
        CreateButton::new("onb:chan")
            .label("Create a channel per county")
            .style(ButtonStyle::Success),
        CreateButton::new("onb:here")
            .label("Use this channel")
            .style(ButtonStyle::Primary),
        CreateButton::new("onb:cancel")
            .label("Cancel")
            .style(ButtonStyle::Danger),
    ];
    (content, vec![CreateActionRow::Buttons(buttons)])
}

// --- helpers -----------------------------------------------------------------

async fn update_message(
    ctx: &Context,
    component: &ComponentInteraction,
    content: String,
    rows: Vec<CreateActionRow>,
) -> anyhow::Result<()> {
    component
        .create_response(
            &ctx.http,
            CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .content(content)
                    .components(rows),
            ),
        )
        .await?;
    Ok(())
}

async fn ephemeral(ctx: &Context, cmd: &CommandInteraction, content: &str) -> anyhow::Result<()> {
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

fn option_str<'a>(cmd: &'a CommandInteraction, name: &str) -> &'a str {
    cmd.data
        .options
        .iter()
        .find(|o| o.name == name)
        .and_then(|o| match &o.value {
            CommandDataOptionValue::String(s) => Some(s.as_str()),
            CommandDataOptionValue::Autocomplete { value, .. } => Some(value.as_str()),
            _ => None,
        })
        .unwrap_or("")
}

fn state_name(code: &str) -> String {
    crate::states::US_STATES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| code.to_string())
}

/// A Discord-safe channel name for a county: `<county>` or `<county>-<suffix>`.
/// e.g. ("Montgomery", "rba") -> "montgomery-rba"; ("Montgomery", "") -> "montgomery".
fn channel_name(county: &str, suffix: &str) -> String {
    let name = if suffix.trim().is_empty() {
        slug(county)
    } else {
        format!("{}-{}", slug(county), slug(suffix))
    };
    truncate(&name, 100)
}

/// Lowercase, hyphenate non-alphanumerics, collapse/trim dashes.
fn slug(s: &str) -> String {
    let mut out: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while out.contains("--") {
        out = out.replace("--", "-");
    }
    out.trim_matches('-').to_string()
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}
