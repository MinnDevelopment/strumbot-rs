[live-event]: https://raw.githubusercontent.com/MinnDevelopment/strumbot-rs/master/assets/live-event.png
[update-event]: https://raw.githubusercontent.com/MinnDevelopment/strumbot-rs/master/assets/update-event.png
[vod-event]: https://raw.githubusercontent.com/MinnDevelopment/strumbot-rs/master/assets/vod-event.png
[rank-joining]: https://raw.githubusercontent.com/MinnDevelopment/strumbot-rs/master/assets/rank-joining.gif
[example-config]: https://github.com/MinnDevelopment/strumbot-rs/blob/master/example-config.json

# strumbot-rs

[![tests](https://github.com/MinnDevelopment/strumbot-rs/actions/workflows/tests.yml/badge.svg)](https://github.com/MinnDevelopment/strumbot-rs/actions/workflows/tests.yml)
[ ![docker-pulls](https://img.shields.io/docker/pulls/minnced/strumbot-rs?logo=docker&logoColor=white) ](https://hub.docker.com/r/minnced/strumbot-rs)
[ ![](https://img.shields.io/docker/image-size/minnced/strumbot-rs?logo=docker&logoColor=white&sort=semver) ](https://hub.docker.com/r/minnced/strumbot-rs/tags?page=1&ordering=last_updated)
[ ![Publish Artifacts](https://github.com/MinnDevelopment/strumbot-rs/actions/workflows/artifact.yml/badge.svg) ](https://github.com/MinnDevelopment/strumbot-rs/actions/workflows/artifact.yml)

A Twitch Stream Notification Bot. This will send notifications to a webhook in your Discord server when the subscribed streamer goes live or changes their game.

## Configurations

The configuration file must be called `config.json` and has to be in the working directory. An example configuration can be found in [`example-config.json`][example-config].

### Discord

This section of the configuration contains settings for the discord side of the bot such as role names and webhook URLs.
Note that the bot uses global role cache, independent of servers, and it is recommended to only have the bot account in one server.

Anything that provides a default or described as optional, can be omitted.

- `server_id` Optional target server id where the bot operates (if it is participant in more than one server at a time)
- `token` The discord bot token
- `stream_notifications` The webhook URL to send stream updates to
- `role_name` Optional configuration of `type`->`role` to change the default names of the update roles (empty value `""` disables the role, and removes the role mention from notifications)
- `enabled_events` Array of events to publish to the `stream_notifications` webhook
- `show_notify_hints` Whether to show a hint in the embed footer about the `/notify` command (default: true)
- `avatar_url` Custom URL for the image to use as the webhook avatar (must be `png`/`jpeg`/`gif`/`webp`, or null)
- `enable_command` Wether the `/notify` command should be enabled (default: true)

The roles used for updates can be managed by the bot with the `/notify role: <type>` command.
This command will automatically assign the role to the user.

For example, with the configuration `"live": "stream is live"` the bot will accept the command `/notify role: live` and assign/remove the role `stream is live` for the user.
These commands are *ephemeral*, which means they only show up to the user who invokes them. This way you can use them anywhere without having any clutter in chat!

![rank-joining.gif][rank-joining]


#### Events

![vod-event.png][vod-event]

- [`live`][live-event] When the streamer goes live
- [`update`][update-event] When the streamer changes the current game
- [`vod`][vod-event] When the streamer goes offline (includes vod timestamps for game changes)

### Twitch

This configuration section contains required information to track the stream status.

- `offline_grace_period` Number of minutes to wait before firing a VOD event after channel appears offline (Default: 2)
- `top_clips` The maximum number of top clips to show in the vod event (0 <= x <= 5, default 0)
- `client_id` The twitch application's client_id
- `client_secret` The twitch application's client_secret
- `user_login` The list of usernames for the individual streamers

The `offline_grace_period` is an engineering parameter which is helpful to handle cases where streams temporarily appear offline due to outages or otherwise unwanted connection issues.

### Cache

This lets you control how the cache should be handled. By default, this bot will write the currently tracked stream information into a `.cache` directory in the current working directory.

The purpose of this cache is to handle persistent state between restarts, allowing the bot gracefully resume the stream updates.

- `enabled` Whether to enable the cache (default: true)

You can omit the entire cache config, to use the recommended defaults.

### Example

```json
{
  "discord": {
    "token": "NjUzMjM1MjY5MzM1NjQ2MjA4.*******.*******",
    "stream_notifications": "https://discord.com/api/webhooks/*******/******",
    "show_notify_hints": true,
    "avatar_url": "https://cdn.discordapp.com/avatars/86699011792191488/e43b5218e073a3ae0e9ff7504243bd32.png",
    "role_name": {
      "live": "live",
      "vod": "vod",
      "update": "update"
    },
    "enabled_events": ["live", "update", "vod"]
  },
  "twitch": {
    "top_clips": 5,
    "offline_grace_period": 2,
    "client_id": "*******",
    "client_secret": "*******",
    "user_login": ["Elajjaz", "Distortion2"]
  }
}
```

# Docker Setup

An example setup would be a folder like this:

```
strumbot/
|- docker-compose.yml
|- config.json
\- cache/
```

The config file should be populated as described above. The cache directory should be an empty directory that will be used as a persistent state database between container restarts.

Your docker-compose.yml should then be configured this way:

```yml
version: "2.2"

services:
    strumbot:
        image: minnced/strumbot-rs:1.2.5
        volumes:
            - ./cache:/app/.cache # The hosted cache directory as a local volume
            - ./config.json:/app/config.json # Your config file is also available inside the container as a volume
        restart: unless-stopped # This will restart the app if it crashes for some reason
        container_name: strumbot # Easy accessible container name
```

Once you have this setup, you can start the service with `docker compose build && docker compose up -d`. This will pull the image from [Docker Hub](https://hub.docker.com/r/minnced/strumbot-rs) and start it as a daemon.

To access the logs, you can use `docker logs strumbot`.
