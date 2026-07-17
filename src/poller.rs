//! Per-guild polling scheduler.
//!
//! Each guild (= one eBird token) is polled on a cadence **derived from how many distinct
//! counties it watches**, so it stays under eBird's ~1000 req/day budget automatically. A
//! county subscribed in several channels is fetched **once** and fanned out to each channel.
//!
//! Scheduling: a light 60s tick checks which guilds are due (per-guild `last_polled` +
//! computed cadence), so runtime subscription changes and the day/night window are picked
//! up without restarts. Dedup state ([`Cursors`]) is keyed per (channel, county) and each
//! key is seeded on first sight, so neither a restart nor a fresh subscription replays the
//! backlog.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::{Local, Timelike};
use serenity::all::*;
use tracing::{info, warn};

use crate::cursor::Cursors;
use crate::ebird::EbirdClient;
use crate::format;
use crate::store::Store;

/// Days of history to scan on the notable endpoint.
const DEFAULT_BACK: u16 = 7;
/// eBird's soft daily request limit per token.
const DAILY_BUDGET_PER_TOKEN: f64 = 1000.0;
/// Fraction of the budget we allow ourselves (headroom for retries / manual use).
const BUDGET_SAFETY: f64 = 0.85;
/// How often the scheduler wakes to check which guilds are due.
const TICK: Duration = Duration::from_secs(60);

/// Day/night polling schedule (host-local time), plus budget-aware cadence stretching.
#[derive(Clone)]
pub struct Schedule {
    pub day_cadence_min: u64,
    pub night_cadence_min: u64,
    pub active_start_hour: u32,
    pub active_end_hour: u32,
}

/// Human-readable summary of the polling plan for a given distinct-county count.
pub struct Plan {
    pub day_min: u64,
    pub night_min: u64,
    pub est_daily_requests: u64,
    pub budget: u64,
    pub active_now: bool,
}

impl Schedule {
    /// Whether `hour` (0-23) is in the active window. Handles overnight windows (start > end).
    fn is_active(&self, hour: u32) -> bool {
        if self.active_start_hour <= self.active_end_hour {
            (self.active_start_hour..self.active_end_hour).contains(&hour)
        } else {
            hour >= self.active_start_hour || hour < self.active_end_hour
        }
    }

    /// Hours in the active window.
    fn active_hours(&self) -> f64 {
        let h = if self.active_start_hour <= self.active_end_hour {
            self.active_end_hour - self.active_start_hour
        } else {
            24 - self.active_start_hour + self.active_end_hour
        };
        h as f64
    }

    /// Effective NIGHT cadence (minutes): the configured backoff, stretched only if night
    /// requests *alone* would exceed the budget (i.e. very large county counts).
    fn effective_night_min(&self, counties: usize) -> u64 {
        if counties == 0 {
            return self.night_cadence_min.max(10);
        }
        let budget = DAILY_BUDGET_PER_TOKEN * BUDGET_SAFETY;
        let night_min = (24.0 - self.active_hours()) * 60.0;
        let cap = if night_min > 0.0 {
            ((counties as f64 * night_min) / budget).ceil() as u64
        } else {
            0
        };
        self.night_cadence_min.max(cap).max(10)
    }

    /// Effective DAY cadence (minutes): the configured day cadence, stretched so that day
    /// **and** night requests together stay under budget. Because night is cheap, it leaves
    /// most of the budget for daytime — so this stretches less than a 24h-flat estimate.
    fn effective_day_min(&self, counties: usize) -> u64 {
        if counties == 0 {
            return self.day_cadence_min.max(10);
        }
        let budget = DAILY_BUDGET_PER_TOKEN * BUDGET_SAFETY;
        let active_min = self.active_hours() * 60.0;
        let night_min = (24.0 - self.active_hours()) * 60.0;
        let night_cycles = night_min / self.effective_night_min(counties) as f64;
        let day_budget = budget - counties as f64 * night_cycles;
        let required = if day_budget <= 0.0 {
            u64::from(u32::MAX)
        } else {
            (active_min / (day_budget / counties as f64)).ceil() as u64
        };
        self.day_cadence_min.max(required).max(10)
    }

    /// Effective cadence for the current mode, given a token's distinct-county count.
    fn cadence_for(&self, counties: usize, active: bool) -> Duration {
        let mins = if active {
            self.effective_day_min(counties)
        } else {
            self.effective_night_min(counties)
        };
        Duration::from_secs(mins * 60)
    }

    /// Summarize the effective cadence and projected daily request load for `counties`.
    /// Used by `/ebird-status`.
    pub fn plan(&self, counties: usize) -> Plan {
        let day_min = self.effective_day_min(counties);
        let night_min = self.effective_night_min(counties);
        let active_h = self.active_hours();
        let night_h = 24.0 - active_h;
        let cycles = active_h * 60.0 / day_min as f64 + night_h * 60.0 / night_min as f64;
        Plan {
            day_min,
            night_min,
            est_daily_requests: (counties as f64 * cycles).round() as u64,
            budget: (DAILY_BUDGET_PER_TOKEN * BUDGET_SAFETY) as u64,
            active_now: self.is_active(Local::now().hour()),
        }
    }
}

pub async fn run(http: Arc<Http>, store: Store, schedule: Schedule) {
    let mut cursors = Cursors::new();
    // guild_id -> when we last polled it.
    let mut last_polled: HashMap<String, Instant> = HashMap::new();
    // (channel:county) keys already seeded, so first sight doesn't replay the backlog.
    let mut seeded: HashSet<String> = HashSet::new();

    loop {
        let now = Instant::now();
        let active = schedule.is_active(Local::now().hour());

        for gp in store.guild_polls().await {
            // Group this guild's subscriptions by county: fetch once, fan out to channels.
            let mut by_county: HashMap<String, (String, Vec<u64>)> = HashMap::new();
            for sub in &gp.subs {
                by_county
                    .entry(sub.region_code.clone())
                    .or_insert_with(|| (sub.name.clone(), Vec::new()))
                    .1
                    .push(sub.channel_id);
            }

            let cadence = schedule.cadence_for(by_county.len(), active);
            let due = last_polled
                .get(&gp.guild_id)
                .is_none_or(|t| now.duration_since(*t) >= cadence);
            if !due {
                continue;
            }
            last_polled.insert(gp.guild_id.clone(), now);

            info!(
                guild = %gp.guild_id,
                counties = by_county.len(),
                cadence_min = cadence.as_secs() / 60,
                active,
                "polling guild"
            );

            let client = EbirdClient::new(gp.token);
            for (region, (name, channels)) in &by_county {
                let mut window = match client.recent_notable(region, DEFAULT_BACK).await {
                    Ok(w) => w,
                    Err(e) => {
                        warn!(region = %region, "poll failed: {e}");
                        continue;
                    }
                };
                // eBird can list the same obsId more than once in a single response; keep
                // only the first of each so we never alert one observation multiple times.
                let mut seen_ids = HashSet::new();
                window.retain(|o| seen_ids.insert(o.obs_id.clone()));

                for &channel_id in channels {
                    let key = format!("{channel_id}:{region}");
                    if seeded.insert(key.clone()) {
                        // First time we've seen this (channel, county): seed, don't alert.
                        cursors.seed(&key, &window);
                        info!(key = %key, count = window.len(), "seeded (no alerts on first sight)");
                    } else {
                        let fresh = cursors.unseen(&key, &window);
                        let mut posted = Vec::new();
                        for obs in fresh {
                            let embed = format::alert_embed(obs, name);
                            match ChannelId::new(channel_id)
                                .send_message(&http, CreateMessage::new().embed(embed))
                                .await
                            {
                                Ok(_) => {
                                    info!(key = %key, obs = %obs.obs_id, "alerted {}", obs.com_name);
                                    posted.push(obs.obs_id.clone());
                                }
                                Err(e) => warn!(key = %key, obs = %obs.obs_id, "post failed: {e}"),
                            }
                        }
                        cursors.mark_seen(&key, posted);
                    }
                    cursors.retain_window(&key, &window);
                }
            }
        }

        tokio::time::sleep(TICK).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sched() -> Schedule {
        Schedule {
            day_cadence_min: 15,
            night_cadence_min: 120,
            active_start_hour: 4,
            active_end_hour: 20,
        }
    }

    #[test]
    fn few_counties_keep_configured_cadence() {
        assert_eq!(sched().cadence_for(5, true), Duration::from_secs(15 * 60));
    }

    #[test]
    fn many_counties_stretch_day_cadence_accounting_for_night() {
        // 30 counties over 16h active + 8h night@120min: night uses 30*4=120 of the 850
        // budget, leaving 730 for day → ceil(960 / (730/30)) = 40-min day cadence.
        assert_eq!(sched().cadence_for(30, true), Duration::from_secs(40 * 60));
    }

    #[test]
    fn night_stays_at_configured_backoff() {
        assert_eq!(sched().cadence_for(30, false), Duration::from_secs(120 * 60));
    }

    #[test]
    fn never_below_ten_minute_floor() {
        assert!(sched().cadence_for(1, true) >= Duration::from_secs(10 * 60));
    }

    #[test]
    fn active_window_boundaries() {
        let s = sched();
        assert!(s.is_active(4) && s.is_active(19));
        assert!(!s.is_active(20) && !s.is_active(3));
    }

    #[test]
    fn plan_estimates_daily_requests() {
        let p = sched().plan(5);
        assert_eq!(p.day_min, 15);
        assert_eq!(p.night_min, 120);
        assert_eq!(p.est_daily_requests, 5 * 68);
        assert_eq!(p.budget, 850);
    }
}
