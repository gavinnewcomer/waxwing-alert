# Terms of Service — Waxwing Alert (eBird RBA)

**Last updated: July 18, 2026**

These Terms of Service ("Terms") govern your use of the **Waxwing Alert** Discord bot ("the bot", "the Service") as operated by Gavin Newcomer ("we", "us"). By adding the bot to a Discord server or using its commands, you ("you", the server administrator and its members) agree to these Terms. If you do not agree, do not use the bot.

## 1. The Service

Waxwing Alert is a free Discord bot that posts notifications about notable and rare bird sightings, by US county, using data from the [eBird API 2.0](https://documenter.getpostman.com/view/664302/S1ENwy59) operated by the Cornell Lab of Ornithology. Server admins activate an eBird API key and subscribe channels to counties; the bot polls eBird on a schedule and posts alerts to the subscribed channels.

## 2. Eligibility and Discord's Terms

You must comply with the [Discord Terms of Service](https://discord.com/terms) and [Community Guidelines](https://discord.com/guidelines) at all times. You must have the necessary permissions in a Discord server to add the bot and to allow it to create, modify, and delete channels as part of its normal operation.

## 3. Bring your own eBird API key

Each server must supply its own eBird API key via `/wwa-activate`. You are responsible for:

- Obtaining your key from [eBird](https://ebird.org/api/keygen) and keeping it valid.
- Complying with **eBird's and the Cornell Lab of Ornithology's terms of use and API usage limits.** Your key is used only to serve your server's subscriptions, and the bot self-limits its request rate to stay under eBird's published daily quota, but you remain responsible for your key's usage and standing with eBird.

We are not affiliated with, endorsed by, or sponsored by eBird or the Cornell Lab of Ornithology.

## 4. Permissions and channel management

To function, the bot requires Discord permissions including View Channels, Send Messages, Embed Links, Manage Channels, and Manage Roles. With your authorization, the bot may **create channels and categories** (during onboarding), set them **read-only**, and **delete the channels and categories it created** (via `/wwa-purge`). The bot only deletes channels it created and tracks; it does not delete channels you made yourself. You are responsible for reviewing these actions before confirming them.

## 5. Acceptable use

You agree not to:

- Use the bot for any unlawful purpose or in violation of Discord's or eBird's terms.
- Attempt to abuse, overload, disrupt, reverse-engineer for the purpose of attacking, or gain unauthorized access to the Service or its infrastructure.
- Use the bot to harass others or to facilitate harm to birds or sensitive sites (for example, targeting sensitive nesting locations). Please respect [eBird's guidance on sensitive species](https://support.ebird.org/en/support/solutions/articles/48000803210) and responsible birding practices.

## 6. Availability — no guarantee

The Service is provided on a **best-effort basis and may be unavailable, delayed, or discontinued at any time**, with or without notice. We do not guarantee any level of uptime, polling frequency, or delivery of any particular alert. Alerts depend on eBird's data and review process, and sightings may be delayed, incomplete, out of order, or missed. **Do not rely on the bot as a sole or authoritative source** for any time-sensitive or safety-related decision.

## 7. Accuracy of data

Sighting information originates from eBird and its contributors and is provided to you as-is. We do not verify, correct, or take responsibility for the accuracy, completeness, or timeliness of any sighting, location, or other data displayed.

## 8. Open-source software

The software behind the bot is open-source and licensed under the **Apache License 2.0**. You are free to run your own copy under that license. These Terms govern the **hosted bot we operate**; if you self-host, you operate that instance yourself and these Terms do not apply to it (the software license does).

## 9. Disclaimer of warranties

THE SERVICE IS PROVIDED "AS IS" AND "AS AVAILABLE", WITHOUT WARRANTIES OF ANY KIND, WHETHER EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO IMPLIED WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, AND NON-INFRINGEMENT. WE DO NOT WARRANT THAT THE SERVICE WILL BE UNINTERRUPTED, TIMELY, SECURE, OR ERROR-FREE.

## 10. Limitation of liability

TO THE MAXIMUM EXTENT PERMITTED BY LAW, IN NO EVENT WILL WE BE LIABLE FOR ANY INDIRECT, INCIDENTAL, SPECIAL, CONSEQUENTIAL, OR PUNITIVE DAMAGES, OR ANY LOSS OF PROFITS, DATA, GOODWILL, OR MISSED SIGHTINGS, ARISING OUT OF OR RELATED TO YOUR USE OF (OR INABILITY TO USE) THE SERVICE. BECAUSE THE SERVICE IS PROVIDED FREE OF CHARGE, OUR TOTAL AGGREGATE LIABILITY FOR ANY CLAIM RELATING TO THE SERVICE WILL NOT EXCEED **USD $0**.

## 11. Termination

You may stop using the Service at any time by removing the bot from your server. We may suspend, restrict, or terminate the Service, or your access to it, at any time and for any reason, including to protect the Service, comply with eBird's or Discord's terms, or if we discontinue the project.

## 12. Changes to these Terms

We may update these Terms from time to time. Material changes will be reflected by updating the "Last updated" date above. Continued use of the bot after a change constitutes acceptance of the revised Terms.

## 13. Governing law

These Terms are governed by the laws of the State of _[your state]_, United States, without regard to its conflict-of-laws rules. _(Set your state of residence here.)_

## 14. Contact

Questions about these Terms: **gavin@waxwing.xyz**.
