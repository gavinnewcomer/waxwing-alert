//! Persistent state: a single JSON file mapping each guild to its (encrypted) eBird
//! token and its county subscriptions. No database.
//!
//! `Store` is a cheap-to-clone handle around `Arc<RwLock<…>>`; the command handler and
//! the poller share one instance. Every mutation writes the file back out.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::crypto;

/// A channel's subscription to a county's notable feed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Subscription {
    pub channel_id: u64,
    /// County-level eBird region code, e.g. "US-PA-091".
    pub region_code: String,
    /// Display name, e.g. "Montgomery". Falls back to the code if unknown.
    pub name: String,
    /// True if the bot created this channel (so `/ebird-purge` may delete it). Channels a
    /// server pointed us at ("use this channel") are false and are never deleted.
    #[serde(default)]
    pub managed: bool,
}

/// A guild's persisted config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GuildState {
    /// Encrypted eBird token (`base64(nonce):base64(ct)`), or `None` until activated.
    pub api_key_enc: Option<String>,
    pub subscriptions: Vec<Subscription>,
    /// Ids of categories the bot created (so `/ebird-purge` can remove them by id, even if
    /// the server renamed or moved them).
    #[serde(default)]
    pub managed_categories: Vec<u64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Persisted {
    guilds: HashMap<String, GuildState>,
}

/// A guild's polling unit: its decrypted token and all its subscriptions. The poller
/// groups these by county (to fetch each county once) and derives a per-token cadence
/// from the distinct-county count.
pub struct GuildPoll {
    pub guild_id: String,
    pub token: String,
    pub subs: Vec<Subscription>,
}

#[derive(Clone)]
pub struct Store {
    inner: Arc<RwLock<Persisted>>,
    path: PathBuf,
    enc_key: [u8; 32],
}

impl Store {
    /// Load the state file (or start empty if it doesn't exist yet).
    pub fn load(path: impl Into<PathBuf>, enc_key: [u8; 32]) -> anyhow::Result<Self> {
        let path = path.into();
        let inner = if path.exists() {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("reading state file {}", path.display()))?;
            serde_json::from_slice(&bytes).context("parsing state file")?
        } else {
            Persisted::default()
        };
        Ok(Self {
            inner: Arc::new(RwLock::new(inner)),
            path,
            enc_key,
        })
    }

    async fn save(&self) -> anyhow::Result<()> {
        let guard = self.inner.read().await;
        let json = serde_json::to_vec_pretty(&*guard)?;
        std::fs::write(&self.path, json)
            .with_context(|| format!("writing state file {}", self.path.display()))?;
        Ok(())
    }

    /// Store (or rotate) a guild's eBird token, encrypted.
    pub async fn activate(&self, guild_id: &str, token: &str) -> anyhow::Result<()> {
        let enc = crypto::encrypt(&self.enc_key, token)?;
        {
            let mut g = self.inner.write().await;
            g.guilds.entry(guild_id.to_string()).or_default().api_key_enc = Some(enc);
        }
        self.save().await
    }

    /// Whether this guild has activated a key.
    pub async fn has_key(&self, guild_id: &str) -> bool {
        self.inner
            .read()
            .await
            .guilds
            .get(guild_id)
            .and_then(|g| g.api_key_enc.as_ref())
            .is_some()
    }

    /// This guild's decrypted eBird token, if activated.
    pub async fn token_for(&self, guild_id: &str) -> anyhow::Result<Option<String>> {
        let g = self.inner.read().await;
        match g.guilds.get(guild_id).and_then(|gs| gs.api_key_enc.as_ref()) {
            Some(enc) => Ok(Some(crypto::decrypt(&self.enc_key, enc)?)),
            None => Ok(None),
        }
    }

    /// Subscribe a channel to a county. `managed` = the bot created this channel. Returns
    /// `true` if newly added.
    pub async fn subscribe(
        &self,
        guild_id: &str,
        channel_id: u64,
        region_code: &str,
        name: &str,
        managed: bool,
    ) -> anyhow::Result<bool> {
        let added = {
            let mut g = self.inner.write().await;
            let gs = g.guilds.entry(guild_id.to_string()).or_default();
            if gs
                .subscriptions
                .iter()
                .any(|s| s.channel_id == channel_id && s.region_code == region_code)
            {
                false
            } else {
                gs.subscriptions.push(Subscription {
                    channel_id,
                    region_code: region_code.to_string(),
                    name: name.to_string(),
                    managed,
                });
                true
            }
        };
        if added {
            self.save().await?;
        }
        Ok(added)
    }

    /// Record a category the bot created, so purge can later remove it by id.
    pub async fn record_category(&self, guild_id: &str, category_id: u64) -> anyhow::Result<()> {
        let changed = {
            let mut g = self.inner.write().await;
            let gs = g.guilds.entry(guild_id.to_string()).or_default();
            if gs.managed_categories.contains(&category_id) {
                false
            } else {
                gs.managed_categories.push(category_id);
                true
            }
        };
        if changed {
            self.save().await?;
        }
        Ok(())
    }

    /// Distinct channel ids the bot created for this guild (drives `/ebird-purge`).
    pub async fn managed_channel_ids(&self, guild_id: &str) -> Vec<u64> {
        let g = self.inner.read().await;
        let Some(gs) = g.guilds.get(guild_id) else {
            return Vec::new();
        };
        let mut ids: Vec<u64> = gs
            .subscriptions
            .iter()
            .filter(|s| s.managed)
            .map(|s| s.channel_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Category ids the bot created for this guild.
    pub async fn managed_category_ids(&self, guild_id: &str) -> Vec<u64> {
        self.inner
            .read()
            .await
            .guilds
            .get(guild_id)
            .map(|gs| gs.managed_categories.clone())
            .unwrap_or_default()
    }

    /// Remove every subscription pointing at a (now-deleted) channel, and drop it from the
    /// managed-category list if it was one of ours. Returns how many subscriptions went.
    pub async fn remove_channel(&self, guild_id: &str, channel_id: u64) -> anyhow::Result<usize> {
        let (removed, changed) = {
            let mut g = self.inner.write().await;
            match g.guilds.get_mut(guild_id) {
                Some(gs) => {
                    let before_subs = gs.subscriptions.len();
                    gs.subscriptions.retain(|s| s.channel_id != channel_id);
                    let removed = before_subs - gs.subscriptions.len();
                    let before_cats = gs.managed_categories.len();
                    gs.managed_categories.retain(|c| *c != channel_id);
                    let changed = removed > 0 || gs.managed_categories.len() != before_cats;
                    (removed, changed)
                }
                None => (0, false),
            }
        };
        if changed {
            self.save().await?;
        }
        Ok(removed)
    }

    /// Remove a channel's subscription to a county. Returns `true` if one was removed.
    pub async fn unsubscribe(
        &self,
        guild_id: &str,
        channel_id: u64,
        region_code: &str,
    ) -> anyhow::Result<bool> {
        let removed = {
            let mut g = self.inner.write().await;
            match g.guilds.get_mut(guild_id) {
                Some(gs) => {
                    let before = gs.subscriptions.len();
                    gs.subscriptions
                        .retain(|s| !(s.channel_id == channel_id && s.region_code == region_code));
                    before != gs.subscriptions.len()
                }
                None => false,
            }
        };
        if removed {
            self.save().await?;
        }
        Ok(removed)
    }

    /// Guild ids that currently have at least one subscription (for the reaper sweep).
    pub async fn guilds_with_subs(&self) -> Vec<String> {
        self.inner
            .read()
            .await
            .guilds
            .iter()
            .filter(|(_, gs)| !gs.subscriptions.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// This guild's subscriptions.
    pub async fn list(&self, guild_id: &str) -> Vec<Subscription> {
        self.inner
            .read()
            .await
            .guilds
            .get(guild_id)
            .map(|g| g.subscriptions.clone())
            .unwrap_or_default()
    }

    /// Active guilds (those with a key AND at least one subscription), tokens decrypted.
    /// This is the guild ↔ token ↔ subscriptions view the poller uses to compute per-token
    /// cadence from the distinct-county count.
    pub async fn guild_polls(&self) -> Vec<GuildPoll> {
        let g = self.inner.read().await;
        let mut out = Vec::new();
        for (guild_id, gs) in g.guilds.iter() {
            if gs.subscriptions.is_empty() {
                continue;
            }
            let Some(enc) = &gs.api_key_enc else { continue };
            let token = match crypto::decrypt(&self.enc_key, enc) {
                Ok(t) => t,
                Err(_) => continue,
            };
            out.push(GuildPoll {
                guild_id: guild_id.clone(),
                token,
                subs: gs.subscriptions.clone(),
            });
        }
        out
    }
}
