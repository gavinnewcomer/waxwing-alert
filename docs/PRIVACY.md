# Privacy Policy — Waxwing Alert (eBird RBA)

**Last updated: July 18, 2026**

This Privacy Policy explains what information the **Waxwing Alert** Discord bot ("the bot", "we", "us") collects, how it is used, and the choices you have. It applies to the **hosted bot** operated by Gavin Newcomer. If you run your own copy of the open-source software, you are the operator of that instance and this policy does not apply to it — see [Self-hosting](#self-hosting) below.

By adding the bot to your Discord server or using its commands, you agree to this policy.

## Summary

Waxwing Alert is deliberately minimal. It does **not** read your messages, build a database of your activity, track your members, or run analytics. The only things it stores are what it needs to send alerts: your server's eBird API key (encrypted) and which channels are subscribed to which counties.

## What we collect

We store only the following, in a single encrypted file on the server that runs the bot:

- **Your eBird API key**, which a server admin provides via `/wwa-activate`. It is **encrypted at rest** (AES-256-GCM) and only decrypted in memory when making requests to eBird. It is never written to logs and cannot be read back through the bot.
- **Discord server (guild) IDs and channel IDs** for the servers and channels that use the bot.
- **County subscriptions** — the eBird region codes (e.g. `US-PA-091`) each channel is subscribed to, and channel-name settings you choose during onboarding.
- **A small amount of internal bookkeeping** needed to run the poller, such as when a server was last checked.

That's the complete list. There is no database of past sightings or activity — the bot is *stateless* by design.

## What we do NOT collect

- **Message content.** The bot does not read, store, or process the messages people post in your server.
- **Personal information about your members** — no usernames, user IDs, email addresses, IP addresses, roles, or profile data are collected or stored.
- **Direct messages.** The bot does not use or read DMs.
- **Analytics or tracking.** There are no tracking pixels, advertising identifiers, or behavioral profiles.

## Information the bot displays (eBird data)

The bot's purpose is to relay **notable bird sightings** from the [eBird API 2.0](https://documenter.getpostman.com/view/664302/S1ENwy59), run by the Cornell Lab of Ornithology. Each alert may include the species, count, location, date/time, and a link to the observer's original eBird checklist. This information originates from eBird and is already publicly available there; the bot simply formats and forwards it to your Discord channels. We do not store this data — it is fetched from eBird, posted, and dropped.

Use of eBird data is governed by eBird's and the Cornell Lab of Ornithology's own terms. Your use of your eBird API key must comply with them.

## How we use your information

We use the stored information solely to operate the service — to authenticate to eBird with your key, determine which counties to check, and post alerts to the correct channels. We do not sell, rent, or share your information with third parties for their own purposes.

## Third-party services

Operating the hosted bot involves a few third parties, each with their own privacy practices:

- **Discord** — the platform the bot runs on. Your use of Discord is subject to [Discord's Privacy Policy](https://discord.com/privacy).
- **eBird / Cornell Lab of Ornithology** — the source of sighting data, accessed with your API key.
- **Hosting and operational logging.** The bot runs on a virtual private server, and operational logs (for reliability and debugging) are forwarded to a log-management provider. These logs record the bot's own runtime events, not your members' messages, and do not include decrypted API keys.

## Data retention and deletion

We keep your eBird key and subscriptions only for as long as the bot is installed and configured on your server. You can remove your data at any time:

- **Remove a subscription** with `/wwa-unsubscribe`, or delete the bot-created channels with `/wwa-purge`.
- **Remove your key and all data** by removing the bot from your server. When the bot is removed, its stored configuration for your server can be deleted on request.
- To request deletion of any data associated with your server, contact us (below) with your Discord server ID.

## Security

Your eBird API key is encrypted at rest and only decrypted in memory when needed. The server is access-restricted (key-based SSH, firewalled). No method of storage or transmission is perfectly secure, but we take reasonable measures to protect the limited data we hold.

## Children's privacy

The bot is not directed at children under 13, and we do not knowingly collect personal information from them. Discord's own age requirements apply.

## Self-hosting

Waxwing Alert is open-source (Apache-2.0). If you run your own instance, **you** are the data controller/operator for that instance: the eBird keys and subscriptions live on your own server, under your control, and this policy does not govern them.

## Changes to this policy

We may update this policy from time to time. Material changes will be reflected by updating the "Last updated" date above. Continued use of the bot after a change constitutes acceptance of the revised policy.

## Contact

Questions or data requests: **gavin@waxwing.xyz**.
