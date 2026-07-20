//! waxwing-alert library crate. The binary (`src/main.rs`) and the integration tests
//! (`tests/`) both build on these modules.

pub mod commands;
pub mod config;
pub mod counties;
pub mod crypto;
pub mod cursor;
pub mod ebird;
pub mod format;
pub mod model;
pub mod onboarding;
pub mod poller;
pub mod purge;
pub mod reaper;
pub mod states;
pub mod store;
