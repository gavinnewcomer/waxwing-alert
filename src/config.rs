//! Runtime configuration from the environment. Subscriptions are set at runtime via slash
//! commands and persisted by [`crate::store`].

use anyhow::Context as _;

use crate::crypto;

pub struct Config {
    pub discord_token: String,
    /// 32-byte key (from hex `EBIRD_ALERT_ENC_KEY`) encrypting stored eBird tokens.
    pub enc_key: [u8; 32],
    /// Path to the JSON state file.
    pub store_path: String,
    /// Poll cadence (minutes) during the active daytime window.
    pub poll_cadence_min: u64,
    /// Poll cadence (minutes) outside the active window (night backoff).
    pub night_poll_cadence_min: u64,
    /// Active window start/end hour in host-local time, e.g. 4 and 20 for 04:00–20:00.
    pub active_start_hour: u32,
    pub active_end_hour: u32,
    /// If set, register slash commands to this one guild (instant, for dev). If `None`,
    /// register globally (production; can take up to ~1h to propagate).
    pub dev_guild_id: Option<u64>,
}

/// Load environment variables from a dotenv file.
///
/// Prefers `.env.local` when it exists (local development — typically sets `DEV_GUILD_ID`
/// so slash commands register to one guild instantly). Otherwise falls back to `.env`
/// (production/deploy — no `DEV_GUILD_ID`, so commands register globally). `.env.local`
/// never ships to the server, so deploys always take the `.env` path.
///
/// Idempotent and never overrides variables already present in the process environment,
/// so it's safe to call from both `main` and [`Config::load`].
pub fn load_env() {
    if std::path::Path::new(".env.local").exists() {
        dotenvy::from_filename(".env.local").ok();
    } else {
        dotenvy::dotenv().ok();
    }
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        load_env();
        Ok(Self {
            discord_token: env("DISCORD_TOKEN")?.trim().to_string(),
            enc_key: crypto::parse_key(&env("EBIRD_ALERT_ENC_KEY")?)?,
            store_path: std::env::var("STORE_PATH")
                .unwrap_or_else(|_| "waxwing-state.json".to_string()),
            poll_cadence_min: parse_or("DEFAULT_POLL_CADENCE_MIN", 15),
            night_poll_cadence_min: parse_or("NIGHT_POLL_CADENCE_MIN", 120),
            active_start_hour: parse_or("ACTIVE_START_HOUR", 4),
            active_end_hour: parse_or("ACTIVE_END_HOUR", 20),
            dev_guild_id: std::env::var("DEV_GUILD_ID")
                .ok()
                .and_then(|s| s.parse().ok()),
        })
    }
}

fn env(key: &str) -> anyhow::Result<String> {
    std::env::var(key).with_context(|| format!("missing required env var: {key}"))
}

/// Parse an optional numeric env var, falling back to `default` if unset or invalid.
fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}
