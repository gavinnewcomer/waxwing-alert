# Waxwing Alert (eBird RBA)

## Overview

Waxwing Alert is a free, open-source tool that sends Discord notifications when someone reports a rare bird in your area. It is built on the [eBird API 2.0](https://documenter.getpostman.com/view/664302/S1ENwy59), the data service run by the Cornell Lab of Ornithology.

**RBA** stands for *Rare Bird Alert*, a notification that an unusual or notable bird has been spotted nearby.

We built Waxwing Alert for birders in the United States, with one main goal: give every state's Discord community equal access to notable-sighting alerts. A single, low-cost server can cover all 50 states, so no community gets left out because of hosting costs.

The tool is *stateless*, meaning it does not store a database of past activity. It simply checks eBird for new reports and passes them along. This keeps it cheap and simple to run. A single server costs about **$5/month** and covers every state at once.

There are two ways to use Waxwing Alert:

- **Use our hosted bot (recommended).** Add the Waxwing Alert bot to your Discord server and follow the guided setup. This is the easiest path and requires no technical skills.
- **Host your own copy.** Run your own instance of the tool. This requires some technical knowledge, though modern AI assistants can walk you through it.

## How does it work?

Waxwing Alert watches for notable bird reports in the regions you choose. Here is the process:

1. **We check eBird regularly.** The bot *polls* the eBird API, meaning it asks eBird, on a repeating schedule, "Are there any new notable sightings?"
2. **We format each new report.** When a new sighting appears, we package it into a clean, readable message. Each alert includes:
   - **Species:** what bird was seen
   - **Count:** how many individuals were reported
   - **Location:** where the bird was spotted
   - **Date and time:** when it was reported
   - **Original checklist:** a link to the birder's full eBird report
3. **We post the alert to the right channel.** The message goes to the Discord channel for that county, so your members are notified about rare birds in the regions they care about.

**How often do we check?** The polling rate, or how frequently we check eBird, depends on three things:

- The number of counties your server subscribes to
- eBird's daily limit on API requests
- The hours of day we poll

You can read more about these limits, and how to increase your polling rate, in [About the eBird API](#about-the-ebird-api) below.

## Who is this for?

Waxwing Alert is for anyone who wants to know when a rare bird shows up nearby.

Our main focus is state birding Discord communities that do not already have a rare-bird alert bot. But the tool works for anyone tracking sightings in specific regions, such as:

- **Big year birders** trying to see as many species as possible in a year
- **Traveling birders** who want alerts for the regions they are passing through

## Getting Started

Follow these steps to add the bot to your Discord server.

### Step 1: Install the bot

Click the link below. Discord will ask you to install the app with the permissions it needs to work:

[Install Waxwing Alert](https://discord.com/oauth2/authorize?client_id=1527488355510521877&permissions=268454928&integration_type=0&scope=applications.commands+bot)

### Step 2: Request your eBird API key

An **API key** is a private password that lets the bot request data from eBird on your behalf. Each server manages its own key, so you will need to request one:

[Request an eBird API key](https://ebird.org/api/keygen)

### Step 3: Run the onboarding commands

Use the two commands below to connect your key and set up your channels.

#### `/wwa-activate`

This command connects your API key to the bot. You will be prompted to paste in the key you requested in Step 2.

Your key is encrypted and stored securely in memory, so no one, including us, can read it back. If you ever need to replace the key, for example if the bot changes owners or you want to rotate the key, just run `/wwa-activate` again and the new key overwrites the old one.

#### `/wwa-onboarding`

This command sets up all your county alert channels at once. You will be asked for two things:

- **Your state**
- **A channel-name suffix:** a short label added to the end of each channel name, so alert channels are easy to tell apart from your regular chat channels. Most admins use `rba`, which produces channel names like `<county_name>-rba`.

Next, you will choose which counties to include. Pick specific ones or select all of them. The bot then creates a channel for each county you chose.

These channels are **read-only by default**, so everyone can subscribe to alerts without worrying about spam from accidental posts.

## About the eBird API

The Cornell Lab of Ornithology sets the rules for how often the bot can request data. Understanding these limits helps explain why polling rates vary from state to state.

### The daily request limit

Cornell allows each API key **1,000 requests per day**. To stay safely under that ceiling, we set our own internal limit of **900 requests per day**, so we never risk crossing the quota.

Each county the bot checks counts as one request. States with many counties can use up their daily limit quickly.

> **Note:** Your normal eBird activity, such as submitting checklists or browsing sightings, does *not* count against this limit. Only the bot's automated checks do.

### Ways to increase your polling rate

If your state has many counties, there are three ways to check more often:

- **Request a higher limit from Cornell.** You can ask Cornell to raise your key's daily limit. They grant these case by case and do not guarantee approval.
- **Combine multiple keys.** If your state has several admins, each can add their own API key. The bot rotates through the keys during the day, which multiplies your total available requests. This helps most for states with many counties.
- **Poll only during daylight hours.** Rare-bird alerts are not very useful overnight, so we only check between **4 AM and 9 PM** in your local time zone. Any sightings reported overnight are caught by the first check the next morning.
