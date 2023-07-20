# last.fm offline scrobbler

[![Build](https://github.com/theirix/offline-scrobbler/actions/workflows/build.yml/badge.svg)](https://github.com/theirix/offline-scrobbler/actions/workflows/build.yml)

**offline-scrobbler** is an utility to scrobble music to Last.fm, which was played offline.

## Installation

    cargo build --release

## Usage

To be written

## Portability

Works on Linux and macOS.

To build a static binary for usage in container use a static CRT linkage:

    cargo build --release --target x86_64-unknown-linux-musl


## License

BSD 3-Clause
