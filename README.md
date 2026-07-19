# waxwing-alert

A self-hosted, **stateless** Discord bot (Rust + [serenity](https://github.com/serenity-rs/serenity))
that alerts channels to **notable/rare bird sightings by US county** using the
[eBird API 2.0](https://documenter.getpostman.com/view/664302/S1ENwy59). Each server
brings its own eBird API key, activated in-app.

**No database.** The alerting hot path holds zero durable state — deduplication lives
entirely in memory, diffed against the live eBird feed each poll. The only thing on disk
is a small encrypted JSON file of keys and subscriptions. That keeps the footprint tiny
(a 1 GB VPS runs it with room to spare) and, as it turns out, makes it *more* correct —
see below.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the design.

## How it works

Server admins activate an eBird key and subscribe channels to counties via slash commands.
The bot polls `GET /v2/data/obs/{regionCode}/recent/notable` per subscription and dedups
against an **in-memory set of `obsId`s** — no database, no dedup table, no write on every
poll. The set is seeded on startup (so a restart doesn't replay the backlog) and pruned as
observations age out of eBird's rolling window, so it stays bounded.

Statelessness here isn't just lean, it's **more correct**. eBird publishes rare sightings
only after human review, so a bird seen days ago can enter the feed *below* newer entries.
A stateful "remember the last id and walk until you hit it" cursor would skip those
late-confirmed rarities; diffing the whole window against an in-memory set can't miss them.

Keys + subscriptions are the one piece of persistent state — a single encrypted JSON file,
not a database. The poll cadence auto-tunes per token to its distinct-county count to stay
under eBird's ~1000 req/day budget, and backs off overnight.

## Slash commands

All admin commands require **Manage Server**; replies are ephemeral.

| Command | Effect |
|---|---|
| `/wwa-activate key:<…>` | Validate + store this server's eBird key (encrypted at rest) |
| `/wwa-fetch-subdivisions state:<…>` | Load a state's counties for autocomplete |
| `/wwa-subscribe state:<…> county:<…>` | Alert **this channel** to a county |
| `/wwa-unsubscribe county:<…>` | Remove the subscription |
| `/wwa-onboarding state:<…> [suffix:<…>]` | Guided setup: pick counties, optionally create channels |
| `/wwa-list` | List this server's subscriptions |
| `/wwa-status` | Cadence + request-budget usage |
| `/wwa-purge` | Delete the channels the bot created |

## 1. Create the Discord application

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications) → **New Application**.
2. **Bot** tab → **Reset Token** → copy it → put it in `.env` as `DISCORD_TOKEN`.
3. **Invite the bot.** Use this URL, replacing `<APP_ID>` with your application's ID
   (OAuth2 tab). Scopes `bot` + `applications.commands`. Permissions `268454928` =
   View Channel + Send Messages + Embed Links + Manage Channels + Manage Roles. Manage
   Channels creates the per-county channels; Manage Roles makes them read-only (deny
   `@everyone` Send Messages):

   ```
   https://discord.com/api/oauth2/authorize?client_id=<APP_ID>&scope=bot%20applications.commands&permissions=268454928
   ```

## 2. Configure + run

```bash
cp .env.example .env
openssl rand -hex 32          # → EBIRD_ALERT_ENC_KEY in .env
# set DISCORD_TOKEN; for instant command registration while testing, set DEV_GUILD_ID
cargo run
```

With `DEV_GUILD_ID` set, commands register to that one guild instantly; unset, they
register globally (up to ~1h to appear). `RUST_LOG` controls log level.

## 3. Use it

In your server: `/wwa-activate key:<your eBird token>`, then `/wwa-onboarding
state:Pennsylvania` (or `/wwa-subscribe`). Get an eBird token at
https://ebird.org/api/keygen.

## Tests

```bash
cargo test                                                            # unit tests (offline)
EBIRD_TEST_TOKEN=your_token cargo test --test ebird_api -- --nocapture  # live eBird calls
```

## Run with Docker

The easiest way to self-host — no Rust toolchain needed. State (encrypted keys +
subscriptions) persists in a named volume.

```bash
cp .env.example .env          # set DISCORD_TOKEN + EBIRD_ALERT_ENC_KEY (openssl rand -hex 32)
docker compose up -d          # builds the image and runs it
docker compose logs -f        # watch it connect
```

Or without Compose:

```bash
docker build -t waxwing-alert .
docker run -d --name waxwing-alert --restart unless-stopped \
  --env-file .env -v waxwing-alert-state:/data waxwing-alert
```

The image is multi-stage (a distroless runtime, ~50 MB). The bot writes its state file to
`/data`, so keep that volume to preserve activations + subscriptions across restarts.

## License

Apache-2.0. See [LICENSE](LICENSE).
