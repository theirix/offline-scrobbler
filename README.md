# last.fm offline scrobbler

[![Crates.io](https://img.shields.io/crates/v/offline_scrobbler.svg)](https://crates.io/crates/offline_scrobbler)
[![Build](https://github.com/theirix/offline-scrobbler/actions/workflows/build.yml/badge.svg)](https://github.com/theirix/offline-scrobbler/actions/workflows/build.yml)

**offline-scrobbler** is a utility to scrobble music to Last.fm without playing it online. If you have played a favourite album on your stereo system or a very secure media player without scrobbling capabilities, **offline-scrobbler** allows you to scrobble it later.

## Installation

    cargo build --release

## Usage

## 1. Create Last.fm application account

1. Navigate to Last.fm [Create API account](https://www.last.fm/api/account/create) page 
2. Fill out the fields "Contact e-mail" and "Application name" to whatever you like. Other fields should be empty". Click "Submit".
3. Grab "API Key" and "Shared secret" from the resulting page. You need them to set up the scrobbler once.

## 2. Setup offline scrobbler

Set up the scrobbler with the following command, replacing `API_KEY` with the API key from the previous step and `SHARED_SECRET` with the shared secret. You only need to do it once.
```sh
offline-scrobbler auth --api-key API_KEY --secret-key SHARED_SECRET
```

The session key is now stored in a configuration file, and the scrobbler is ready to work. To reset authentication, remove the config file from the [standard path](https://docs.rs/directories/latest/directories/struct.ProjectDirs.html#examples) "~/Library/Application Support/ru.omniverse.offline-scrobbler/config.toml" on macOS or "~/.config/ru.omniverse.offline-scrobbler" on Linux

## 3. Scrobble

There are different modes of scrobbler:
- scrobble a track
- scrobble whole album
- scrobble album given Last.fm album URL

To scrobble an album, call
```sh
offline-scrobbler scrobble --artist=Hooverphonic --album="A New Stereophonic Sound Spectacular"
```

To scrobble using a URL with the specific Last.fm album, call
```sh
offline-scrobbler scrobble-url --url "https://www.last.fm/music/Hooverphonic/Blue+Wonder+Power+Milk"
```

To scrobble a single track of artist (no album), call
```sh
offline-scrobbler scrobble --artist=Hooverphonic --track=Eden
```

The valuable feature of scrobble is the ability to scrobble to the past.
For example, you have listened to a track one hour ago.  Then you can specify additional argument `--start=1h` or ``--start=60m`` or even `--start="1h 15min"`! Formats are described [here](https://docs.rs/humantime/latest/humantime/fn.parse_duration.html). It is a scrobbler.

For simplicity, when you invoke scrobbling of an album, the scrobbler analyses all tracks' duration in the album and scrobbles them sequentially until the current moment. Therefore, when you launch the scrobbler, the album will be scrobbled as if you just finished listening to it for an hour.

## Portability

Works on Linux and macOS.

To build a Linux static binary with minimal dependencies, use a static CRT linkage:

    cargo build --release --target x86_64-unknown-linux-musl


## License

BSD 3-Clause
