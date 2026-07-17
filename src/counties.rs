//! Lazy per-state county cache. A state's counties are fetched from eBird only the first
//! time someone asks for that state (via `/ebird-fetch-subdivisions`), then cached in
//! memory. States nobody engages are never fetched.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::ebird::EbirdClient;

#[derive(Clone, Default)]
pub struct CountyCache {
    /// state region code -> shared list of `(county_code, county_name)`.
    inner: Arc<RwLock<HashMap<String, Arc<Vec<(String, String)>>>>>,
}

impl CountyCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Counties for a state — cached after the first fetch. `(code, name)` pairs.
    pub async fn get_or_load(
        &self,
        state_code: &str,
        client: &EbirdClient,
    ) -> anyhow::Result<Arc<Vec<(String, String)>>> {
        if let Some(hit) = self.inner.read().await.get(state_code).cloned() {
            return Ok(hit);
        }
        let counties = Arc::new(client.list_counties(state_code).await?);
        self.inner
            .write()
            .await
            .insert(state_code.to_string(), counties.clone());
        Ok(counties)
    }

    /// Read a state's counties from the cache without fetching. Autocomplete uses this so
    /// it never makes a network call inside Discord's ~3s response window.
    pub async fn get_cached(&self, state_code: &str) -> Option<Arc<Vec<(String, String)>>> {
        self.inner.read().await.get(state_code).cloned()
    }

    /// Look up a county's display name from the cache (no fetch). Used to label a
    /// subscription nicely at `/ebird-subscribe` time (the cache is warm from a preload).
    pub async fn name_for(&self, state_code: &str, county_code: &str) -> Option<String> {
        let map = self.inner.read().await;
        let list = map.get(state_code)?;
        list.iter()
            .find(|(code, _)| code == county_code)
            .map(|(_, name)| name.clone())
    }
}
