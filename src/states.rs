//! Static list of US states/territories for the `state` autocomplete, keyed by eBird
//! subnational1 region code.

/// `(region_code, display_name)` for each US state + DC.
pub const US_STATES: &[(&str, &str)] = &[
    ("US-AL", "Alabama"),
    ("US-AK", "Alaska"),
    ("US-AZ", "Arizona"),
    ("US-AR", "Arkansas"),
    ("US-CA", "California"),
    ("US-CO", "Colorado"),
    ("US-CT", "Connecticut"),
    ("US-DE", "Delaware"),
    ("US-DC", "District of Columbia"),
    ("US-FL", "Florida"),
    ("US-GA", "Georgia"),
    ("US-HI", "Hawaii"),
    ("US-ID", "Idaho"),
    ("US-IL", "Illinois"),
    ("US-IN", "Indiana"),
    ("US-IA", "Iowa"),
    ("US-KS", "Kansas"),
    ("US-KY", "Kentucky"),
    ("US-LA", "Louisiana"),
    ("US-ME", "Maine"),
    ("US-MD", "Maryland"),
    ("US-MA", "Massachusetts"),
    ("US-MI", "Michigan"),
    ("US-MN", "Minnesota"),
    ("US-MS", "Mississippi"),
    ("US-MO", "Missouri"),
    ("US-MT", "Montana"),
    ("US-NE", "Nebraska"),
    ("US-NV", "Nevada"),
    ("US-NH", "New Hampshire"),
    ("US-NJ", "New Jersey"),
    ("US-NM", "New Mexico"),
    ("US-NY", "New York"),
    ("US-NC", "North Carolina"),
    ("US-ND", "North Dakota"),
    ("US-OH", "Ohio"),
    ("US-OK", "Oklahoma"),
    ("US-OR", "Oregon"),
    ("US-PA", "Pennsylvania"),
    ("US-RI", "Rhode Island"),
    ("US-SC", "South Carolina"),
    ("US-SD", "South Dakota"),
    ("US-TN", "Tennessee"),
    ("US-TX", "Texas"),
    ("US-UT", "Utah"),
    ("US-VT", "Vermont"),
    ("US-VA", "Virginia"),
    ("US-WA", "Washington"),
    ("US-WV", "West Virginia"),
    ("US-WI", "Wisconsin"),
    ("US-WY", "Wyoming"),
];

/// Autocomplete choices `(display_name, region_code)` matching `partial` (case-insensitive
/// on name or code), capped at Discord's 25-choice limit.
pub fn filter(partial: &str) -> Vec<(String, String)> {
    let p = partial.to_lowercase();
    US_STATES
        .iter()
        .filter(|(code, name)| {
            p.is_empty() || name.to_lowercase().contains(&p) || code.to_lowercase().contains(&p)
        })
        .take(25)
        .map(|(code, name)| (name.to_string(), code.to_string()))
        .collect()
}
