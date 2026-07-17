//! In-memory, per-county dedup cursor — the whole point of the "no database" design.
//!
//! State lives only in RAM: for each eBird region code we keep the set of `obsId`s we've
//! already surfaced. On startup we [`seed`](Cursors::seed) each region (record the current
//! window as already-seen *without* alerting) so a restart doesn't replay the backlog.
//! Each poll then diffs the fresh window against the set — [`unseen`](Cursors::unseen) —
//! and the caller [`mark_seen`](Cursors::mark_seen)s only what it successfully posted.
//!
//! Why a *set* and not a single "last obsId" high-water mark: eBird's notable feed is
//! ordered by observation date, but records enter it only after human review. A rarity
//! observed days ago but confirmed today appears *below* newer-dated entries, so a
//! "walk down until you hit the last id" scan would skip it. Diffing the whole window
//! can't miss these out-of-order confirmations.

use std::collections::{HashMap, HashSet};

use crate::model::NotableObs;

#[derive(Default)]
pub struct Cursors {
    /// region code -> set of obsIds already surfaced (bounded to the rolling window).
    seen: HashMap<String, HashSet<String>>,
}

impl Cursors {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cold-start seed: mark everything currently in the window as already-seen, so we
    /// don't alert the existing backlog on boot. Call once per region before polling.
    pub fn seed(&mut self, region: &str, window: &[NotableObs]) {
        let set = self.seen.entry(region.to_string()).or_default();
        for o in window {
            set.insert(o.obs_id.clone());
        }
    }

    /// Observations in `window` we have not yet surfaced for this region. Does not mutate;
    /// call [`mark_seen`](Self::mark_seen) after the alert is actually delivered.
    pub fn unseen<'a>(&self, region: &str, window: &'a [NotableObs]) -> Vec<&'a NotableObs> {
        match self.seen.get(region) {
            Some(set) => window.iter().filter(|o| !set.contains(&o.obs_id)).collect(),
            None => window.iter().collect(),
        }
    }

    /// Record obsIds as surfaced. Separate from [`unseen`](Self::unseen) so callers can
    /// mark only what they successfully posted (an alert lost to a Discord error can retry).
    pub fn mark_seen(&mut self, region: &str, ids: impl IntoIterator<Item = String>) {
        self.seen.entry(region.to_string()).or_default().extend(ids);
    }

    /// Bound memory: forget obsIds no longer present in the current window (they've aged
    /// out of eBird's `back` window and won't reappear). Call after each poll.
    pub fn retain_window(&mut self, region: &str, window: &[NotableObs]) {
        if let Some(set) = self.seen.get_mut(region) {
            let live: HashSet<&str> = window.iter().map(|o| o.obs_id.as_str()).collect();
            set.retain(|id| live.contains(id.as_str()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(id: &str) -> NotableObs {
        NotableObs {
            obs_id: id.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn seed_suppresses_startup_backlog() {
        let mut c = Cursors::new();
        let window = vec![obs("OBS1"), obs("OBS2")];
        c.seed("US-PA-091", &window);
        assert!(c.unseen("US-PA-091", &window).is_empty());
    }

    #[test]
    fn detects_new_then_dedups_after_mark() {
        let mut c = Cursors::new();
        c.seed("R", &[obs("OBS1")]);
        let window = vec![obs("OBS1"), obs("OBS2"), obs("OBS3")];

        let fresh: Vec<String> = c.unseen("R", &window).iter().map(|o| o.obs_id.clone()).collect();
        assert_eq!(fresh, ["OBS2", "OBS3"]);

        c.mark_seen("R", fresh);
        assert!(c.unseen("R", &window).is_empty());
    }

    #[test]
    fn catches_late_reviewed_out_of_order_observation() {
        // A rarity confirmed late shows up with an older obsId, not at the top of the feed.
        let mut c = Cursors::new();
        c.seed("R", &[obs("OBS10")]);
        let window = vec![obs("OBS10"), obs("OBS4")]; // OBS4 reviewed after OBS10
        let fresh: Vec<String> = c.unseen("R", &window).iter().map(|o| o.obs_id.clone()).collect();
        assert_eq!(fresh, ["OBS4"]);
    }

    #[test]
    fn retain_window_forgets_aged_out_ids() {
        let mut c = Cursors::new();
        c.seed("R", &[obs("OBS1"), obs("OBS2")]);
        c.retain_window("R", &[obs("OBS2")]);
        assert_eq!(c.unseen("R", &[obs("OBS2")]).len(), 0);
        assert_eq!(c.unseen("R", &[obs("OBS1")]).len(), 1);
    }
}
