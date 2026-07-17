//! Discord embed formatting for notable sightings.

use serenity::all::CreateEmbed;

use crate::model::NotableObs;

/// Build the alert embed for a single notable observation.
pub fn alert_embed(obs: &NotableObs, region_name: &str) -> CreateEmbed {
    let count = obs
        .how_many
        .map(|n| n.to_string())
        .unwrap_or_else(|| "X".to_string());
    let map_url = format!("https://www.google.com/maps?q={},{}", obs.lat, obs.lng);
    let checklist_url = format!("https://ebird.org/checklist/{}", obs.sub_id);

    CreateEmbed::new()
        .title(format!("{} ({})", obs.com_name, obs.sci_name))
        .url(checklist_url.clone())
        .description(format!(
            "**{count}** reported in {region_name}\n\
             📍 [{}]({map_url})\n\
             🗓 {}\n\
             🔗 [Checklist]({checklist_url})",
            obs.loc_name, obs.obs_dt,
        ))
}
