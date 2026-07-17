//! Periodic reaper: prunes subscriptions whose Discord channel no longer exists.
//!
//! The `channel_delete` gateway handler covers deletions that happen while the bot is
//! running; this sweep catches the ones it slept through. Per guild it fetches the channel
//! list in a single API call and removes subscriptions whose channel isn't in it — so a
//! transient fetch error (or the bot being removed from a guild) never prunes anything.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use serenity::all::*;
use tracing::{info, warn};

use crate::store::Store;

/// Wait after startup before the first sweep (let the gateway settle).
const STARTUP_DELAY: Duration = Duration::from_secs(60);
/// Interval between sweeps.
const SWEEP_INTERVAL: Duration = Duration::from_secs(60 * 60);

pub async fn run(http: Arc<Http>, store: Store) {
    tokio::time::sleep(STARTUP_DELAY).await;
    loop {
        sweep(&http, &store).await;
        tokio::time::sleep(SWEEP_INTERVAL).await;
    }
}

async fn sweep(http: &Arc<Http>, store: &Store) {
    for guild_id in store.guilds_with_subs().await {
        let Ok(gid) = guild_id.parse::<u64>().map(GuildId::new) else {
            continue;
        };

        // One call lists all of the guild's channels (regardless of the bot's per-channel
        // view perms). Only prune against a *successful* fetch.
        let live: HashSet<u64> = match gid.channels(http).await {
            Ok(map) => map.keys().map(|id| id.get()).collect(),
            Err(e) => {
                warn!(guild = %guild_id, "reaper: couldn't list channels: {e}");
                continue;
            }
        };

        let dead: HashSet<u64> = store
            .list(&guild_id)
            .await
            .iter()
            .map(|s| s.channel_id)
            .filter(|c| !live.contains(c))
            .collect();

        for channel in dead {
            match store.remove_channel(&guild_id, channel).await {
                Ok(n) if n > 0 => {
                    info!(guild = %guild_id, channel, pruned = n, "reaper: removed subs for unreachable channel")
                }
                Ok(_) => {}
                Err(e) => warn!(guild = %guild_id, channel, "reaper: prune failed: {e}"),
            }
        }
    }
}
