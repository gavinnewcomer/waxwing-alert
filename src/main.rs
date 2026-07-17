use ebird_alert::config::Config;
use ebird_alert::counties::CountyCache;
use ebird_alert::onboarding::OnboardSessions;
use ebird_alert::store::Store;
use ebird_alert::{commands, onboarding, poller, purge, reaper};

use serenity::all::*;
use tracing::{error, info};

struct Handler {
    store: Store,
    counties: CountyCache,
    sessions: OnboardSessions,
    schedule: poller::Schedule,
    dev_guild_id: Option<u64>,
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("connected as {}", ready.user.name);
        let cmds = commands::commands();
        let result = match self.dev_guild_id {
            Some(id) => {
                info!(guild = id, "registering guild commands (instant)");
                GuildId::new(id).set_commands(&ctx.http, cmds).await.map(|_| ())
            }
            None => {
                info!("registering global commands (may take up to ~1h to appear)");
                Command::set_global_commands(&ctx.http, cmds).await.map(|_| ())
            }
        };
        if let Err(e) = result {
            error!("failed to register commands: {e}");
        }
    }

    async fn channel_delete(
        &self,
        _ctx: Context,
        channel: GuildChannel,
        _messages: Option<Vec<Message>>,
    ) {
        let guild_id = channel.guild_id.get().to_string();
        match self.store.remove_channel(&guild_id, channel.id.get()).await {
            Ok(n) if n > 0 => {
                info!(channel = channel.id.get(), pruned = n, "channel deleted — removed its subscriptions")
            }
            Ok(_) => {}
            Err(e) => error!("failed to prune deleted channel's subscriptions: {e}"),
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(cmd) => {
                let result = match cmd.data.name.as_str() {
                    "ebird-onboarding" => {
                        onboarding::start(&ctx, &cmd, &self.store, &self.counties, &self.sessions)
                            .await
                    }
                    "ebird-status" => {
                        commands::status(&ctx, &cmd, &self.store, &self.schedule).await
                    }
                    "ebird-purge" => purge::start(&ctx, &cmd, &self.store).await,
                    _ => commands::dispatch(&ctx, &cmd, &self.store, &self.counties).await,
                };
                if let Err(e) = result {
                    error!("command `{}` failed: {e}", cmd.data.name);
                }
            }
            Interaction::Autocomplete(cmd) => {
                if let Err(e) = commands::autocomplete(&ctx, &cmd, &self.counties).await {
                    error!("autocomplete for `{}` failed: {e}", cmd.data.name);
                }
            }
            Interaction::Component(component) => {
                let result = if component.data.custom_id.starts_with("purge:") {
                    purge::handle_component(&ctx, &component, &self.store).await
                } else {
                    onboarding::handle_component(&ctx, &component, &self.store, &self.sessions).await
                };
                if let Err(e) = result {
                    error!("component `{}` failed: {e}", component.data.custom_id);
                }
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env before initializing tracing so RUST_LOG from it takes effect.
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ebird_alert=info,serenity=warn".into()),
        )
        .init();

    let config = Config::load()?;
    let store = Store::load(&config.store_path, config.enc_key)?;
    let counties = CountyCache::new();
    let schedule = poller::Schedule {
        day_cadence_min: config.poll_cadence_min,
        night_cadence_min: config.night_poll_cadence_min,
        active_start_hour: config.active_start_hour,
        active_end_hour: config.active_end_hour,
    };

    let intents = GatewayIntents::GUILDS;
    let mut client = Client::builder(&config.discord_token, intents)
        .event_handler(Handler {
            store: store.clone(),
            counties,
            sessions: OnboardSessions::new(),
            schedule: schedule.clone(),
            dev_guild_id: config.dev_guild_id,
        })
        .await?;

    let http = client.http.clone();
    tokio::spawn(reaper::run(http.clone(), store.clone()));
    tokio::spawn(poller::run(http, store, schedule));

    info!("starting client");
    if let Err(e) = client.start().await {
        error!("client error: {e}");
    }

    Ok(())
}
