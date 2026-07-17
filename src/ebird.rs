//! eBird API 2.0 client.
//!
//! Endpoints:
//!   - GET /data/obs/{regionCode}/recent/notable?detail=full
//!   - GET /ref/region/list/subnational2/{stateCode}
//!
//! Auth: header `x-ebirdapitoken: <token>`. Soft rate limit ~1000 req/day per token;
//! since each guild supplies its own token, the budget is per-token.

use crate::model::{County, NotableObs};

const BASE: &str = "https://api.ebird.org/v2";

pub struct EbirdClient {
    http: reqwest::Client,
    token: String,
}

impl EbirdClient {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            token: token.into(),
        }
    }

    /// Recent notable (rare/out-of-range/first-of-season) observations for a region.
    pub async fn recent_notable(
        &self,
        region: &str,
        _back: u16,
    ) -> anyhow::Result<Vec<NotableObs>> {
        let notables = self
            .http
            .get(format!("{BASE}/data/obs/{region}/recent/notable?detail=full"))
            .header("x-ebirdapitoken", &self.token)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<NotableObs>>()
            .await?;
        Ok(notables)
    }

    /// List (regionCode, name) counties within a state.
    pub async fn list_counties(&self, state_code: &str) -> anyhow::Result<Vec<(String, String)>> {
        let counties = self
            .http
            .get(format!("{BASE}/ref/region/list/subnational2/{state_code}"))
            .header("x-ebirdapitoken", &self.token)
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<County>>()
            .await?;
        Ok(counties.into_iter().map(|c| (c.code, c.name)).collect())
    }
}
