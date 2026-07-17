use serde::Deserialize;

/// A notable observation from eBird
/// `GET /v2/data/obs/{regionCode}/recent/notable?detail=full`.
///
/// eBird designates "notable" via reviewer-curated per-county regional filters and
/// human review, so this is a stream of *confirmed* rarities (with an inherent review
/// lag). `obs_id` is the stable, unique dedup key.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct NotableObs {
    #[serde(rename = "speciesCode")]
    pub species_code: String,
    #[serde(rename = "comName")]
    pub com_name: String,
    #[serde(rename = "sciName")]
    pub sci_name: String,
    #[serde(rename = "locId")]
    pub loc_id: String,
    #[serde(rename = "locName")]
    pub loc_name: String,
    /// Local datetime "YYYY-MM-DD HH:mm" (time may be absent).
    #[serde(rename = "obsDt")]
    pub obs_dt: String,
    #[serde(rename = "howMany", default)]
    pub how_many: Option<i64>,
    pub lat: f64,
    pub lng: f64,
    #[serde(rename = "obsValid")]
    pub obs_valid: bool,
    #[serde(rename = "obsReviewed")]
    pub obs_reviewed: bool,
    #[serde(rename = "locationPrivate", default)]
    pub location_private: bool,
    /// Checklist id, e.g. "S372739335".
    #[serde(rename = "subId")]
    pub sub_id: String,
    /// Stable per-observation id, e.g. "OBS4682035016" — THE dedup cursor key.
    /// (A single checklist/`subId` can contain several notable species, so we key on this.)
    #[serde(rename = "obsId")]
    pub obs_id: String,
    #[serde(rename = "subnational2Code", default)]
    pub county_code: String,
    #[serde(rename = "userDisplayName", default)]
    pub user_display_name: String,
}

/// A county as returned by eBird's `ref/region/list/subnational2` endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct County {
    pub code: String,
    pub name: String,
}
