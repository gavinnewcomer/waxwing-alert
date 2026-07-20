# waxwing-alert — Architecture

A self-hosted Discord bot (Rust + [serenity](https://github.com/serenity-rs/serenity))
that alerts channels to **notable/rare bird sightings per US county** via the eBird API.
Subscriptions are set at runtime via slash commands; each server brings its own eBird key.

---

## 1. How eBird publishes "notable"

- Every county has a **regional filter** — a reviewer-curated list of expected species/
  counts per date. This *is* the per-county rare-bird definition; we don't build our own.
- A submitted observation is auto-checked against that filter; if unexpected / out-of-
  season / over-count it's **flagged** and held for a **human reviewer**.
- Flagged rares publish only **after review** (lag: minutes → days). So the feed is
  "confirmed notable birds in county X", and polling faster than ~10 min buys nothing.
- Endpoint: `GET https://api.ebird.org/v2/data/obs/{regionCode}/recent/notable?detail=full`
  (auth header `x-ebirdapitoken`). Each obs carries a stable **`obsId`** — the dedup key
  (a checklist/`subId` can hold several notable species). Rate limit ~1000 req/day/token.

---

## 2. Dedup — in-memory `obsId` cursor (no database)

[`src/cursor.rs`](src/cursor.rs): `HashMap<key, HashSet<obsId>>`, keyed per (channel, county).

- **Seeded on first sight** of each key (record current window as seen, no alerts) — so
  neither a restart nor a fresh subscription replays the backlog.
- Each poll takes the **set difference** by `obsId` (`unseen`), posts the fresh ones, and
  `mark_seen`s only what actually posted.
- A set (not a single high-water id) because the feed is ordered by observation date but
  records enter it after review, so late-confirmed rares appear out of order.
- Each fetched window is de-duplicated by `obsId` first (eBird can repeat ids in one
  response).

---

## 3. Persistence + secrets (no database)

[`src/store.rs`](src/store.rs): a single JSON file. `Store` is a cloneable
`Arc<RwLock<…>>` shared by the command handler, poller, and reaper.

```
guilds: { <guild_id>: {
  api_key_enc,                      # AES-256-GCM, key from EBIRD_ALERT_ENC_KEY
  subscriptions: [{ channel_id, region_code, name, managed }],
  managed_categories: [<id>],       # categories the bot created
}}
```

- Tokens encrypted at rest ([`src/crypto.rs`](src/crypto.rs)); decrypted only in memory.
- `managed` distinguishes bot-created channels (purgeable) from a server's own.

---

## 4. Polling ([`src/poller.rs`](src/poller.rs))

- Per **guild** (= token), on a cadence **derived from its distinct-county count** so daily
  requests stay under 85% of the ~1000/day budget. Counties are grouped so each is fetched
  **once** and fanned out to all subscribed channels.
- **Day/night schedule** (host-local `TZ`): full cadence in an active window
  (`ACTIVE_START_HOUR`–`ACTIVE_END_HOUR`), a longer backoff overnight. The budget math
  accounts for the cheap night, letting the day cadence stay faster.
- A 60s tick checks each guild's `last_polled` vs. its cadence, so runtime subscription
  changes and the day/night transition apply without restarts.

---

## 5. Commands ([`src/commands.rs`](src/commands.rs), [`onboarding.rs`](src/onboarding.rs), [`purge.rs`](src/purge.rs))

activate (validated) · fetch-subdivisions · subscribe/unsubscribe (state→county
autocomplete, cache-only) · onboarding (paginated multi-select, Select all, custom channel
naming, read-only channels nested under auto-spilling "eBird Alerts" categories) · list ·
status · purge (deletes bot-created channels by recorded id).

## 6. Subscription hygiene

Three layers keep the store honest: duplicate rejection on subscribe, a `channel_delete`
gateway handler that prunes instantly, and a periodic **reaper** ([`src/reaper.rs`](src/reaper.rs))
that sweeps hourly for channels deleted while the bot was offline.

---

## 7. Decisions locked

- Rust + serenity; **no database** — encrypted JSON file + in-memory `obsId` cursor.
- Per-token dynamic cadence with day/night backoff, budget-aware.
- Each server activates its own eBird key at runtime; bot-created alert channels are
  read-only. Self-hosted; Apache-2.0.
