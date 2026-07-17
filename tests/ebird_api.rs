//! Integration tests that hit the **real** eBird API.
//!
//! Gated on the `EBIRD_TEST_TOKEN` env var (also read from `.env`): if it is unset, each
//! test prints a skip notice and passes, so `cargo test` stays green without a token.
//!
//! Run them with a token:
//!   EBIRD_TEST_TOKEN=your_token cargo test --test ebird_api -- --nocapture
//!
//! Get a token at https://ebird.org/api/keygen

use ebird_alert::ebird::EbirdClient;

/// The eBird token to test with, or `None` to signal the caller to skip. Reads `.env` so
/// you can keep the token out of your shell profile.
fn test_token() -> Option<String> {
    let _ = dotenvy::dotenv();
    std::env::var("EBIRD_TEST_TOKEN")
        .ok()
        .filter(|s| !s.is_empty())
}

/// Skip helper: returns the token or prints a notice and returns early.
macro_rules! token_or_skip {
    ($name:literal) => {
        match test_token() {
            Some(t) => t,
            None => {
                eprintln!(concat!("SKIP ", $name, ": set EBIRD_TEST_TOKEN to run"));
                return;
            }
        }
    };
}

#[tokio::test]
async fn list_counties_returns_pennsylvania_counties() {
    let token = token_or_skip!("list_counties_returns_pennsylvania_counties");

    let client = EbirdClient::new(token);
    let counties = client
        .list_counties("US-PA")
        .await
        .expect("eBird list_counties request failed");

    assert!(!counties.is_empty(), "expected US-PA to have counties");
    assert!(
        counties.iter().all(|(code, _)| code.starts_with("US-PA-")),
        "every county code should be under US-PA, got {counties:?}",
    );
    assert!(
        counties
            .iter()
            .any(|(code, name)| code == "US-PA-091" && name.contains("Montgomery")),
        "Montgomery County (US-PA-091) should be present, got {counties:?}",
    );
}

#[tokio::test]
async fn recent_notable_returns_well_formed_observations() {
    let token = token_or_skip!("recent_notable_returns_well_formed_observations");

    let client = EbirdClient::new(token);
    let obs = client
        .recent_notable("US-PA-091", 30)
        .await
        .expect("eBird recent_notable request failed");

    // Notable birds aren't guaranteed on any given day, so an empty result is a valid pass.
    // What we assert is that whatever comes back is well-formed for our dedup + formatting.
    for o in &obs {
        assert!(!o.obs_id.is_empty(), "every observation needs an obsId (our dedup key)");
        assert!(!o.com_name.is_empty(), "every observation needs a common name");
        assert!(!o.sub_id.is_empty(), "every observation needs a checklist id");
    }
}
