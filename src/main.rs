mod auth;
mod lastfmapi;
mod scrobbler;
mod utils;

use crate::auth::authenticate;
use crate::scrobbler::{scrobble_album, scrobble_track, scrobble_url};
use anyhow::Context;
use clap::Parser;
use env_logger::Env;
use log::{error, info};
use time::Duration;

#[derive(Debug, Clone, Parser)]
enum CliArgs {
    #[command(about = "Scrobble album of artist or track of artist to Last.fm")]
    Scrobble {
        /// Artist name
        #[arg(long)]
        artist: String,

        /// Album name
        #[arg(long)]
        album: Option<String>,

        /// Track name
        #[arg(long)]
        track: Option<String>,

        /// Dry run mode (no writes done)
        #[arg(short, long)]
        dryrun: bool,

        /// Start time
        #[arg(long)]
        start: Option<String>,
    },

    #[command(about = "Scrobble album from given URL to Last.fm")]
    ScrobbleUrl {
        /// Last.fm album page URL
        #[arg(long)]
        url: String,

        /// Dry run mode (no writes done)
        #[arg(short, long)]
        dryrun: bool,

        /// Start time
        #[arg(long)]
        start: Option<String>,
    },

    #[command(about = "Authenticate with Last.fm desktop API")]
    Auth {
        /// API key
        #[arg(long)]
        api_key: String,

        /// Secret key
        #[arg(long)]
        secret_key: String,
    },
}

fn start_to_duration(arg: Option<String>) -> anyhow::Result<Option<Duration>> {
    let opt_duration = match arg {
        Some(sduration) => {
            let u = humantime::parse_duration(&sduration).context("Parse string start time")?;
            let duration: Duration = Duration::try_from(u)?;
            Some(duration)
        }
        None => None,
    };
    Ok(opt_duration)
}

fn run(cli_args: CliArgs) -> anyhow::Result<()> {
    match cli_args {
        CliArgs::Auth {
            api_key,
            secret_key,
        } => authenticate(api_key, secret_key),
        CliArgs::Scrobble {
            artist,
            album,
            track: _,
            dryrun,
            start,
        } if album.is_some() => {
            scrobble_album(artist, album.unwrap(), dryrun, start_to_duration(start)?)
        }
        CliArgs::Scrobble {
            artist,
            album: _,
            track,
            dryrun,
            start,
        } if track.is_some() => {
            scrobble_track(artist, track.unwrap(), dryrun, start_to_duration(start)?)
        }
        CliArgs::Scrobble { .. } => {
            anyhow::bail!("Wrong arguments");
        }
        CliArgs::ScrobbleUrl { url, dryrun, start } => {
            scrobble_url(url, dryrun, start_to_duration(start)?)
        }
    }
}

/// Entry point
fn main() -> Result<(), anyhow::Error> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug"))
        .write_style(if atty::is(atty::Stream::Stdout) {
            env_logger::WriteStyle::Auto
        } else {
            env_logger::WriteStyle::Never
        })
        .format_timestamp(None)
        .init();

    let cli_args = CliArgs::parse();
    let result = run(cli_args);
    match result {
        Ok(_) => {
            info!("Done");
            Ok(())
        }
        Err(err) => {
            error!("Error: {}", err);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use test_log::test;

    #[test]
    fn test_duration() {
        assert!(start_to_duration(Some("1h".to_string())).is_ok());
        assert!(start_to_duration(Some("1h".to_string())).unwrap().is_some());
        assert!(start_to_duration(Some("30minutes".to_string())).is_ok());
        assert!(start_to_duration(Some("-1h".to_string())).is_err());
    }
}
