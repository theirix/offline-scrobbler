mod auth;
mod lastfmapi;
mod scrobbler;

use crate::auth::authenticate;
use crate::scrobbler::{scrobble_album, scrobble_track};
use env_logger::Env;
use log::{error, info};
use structopt::StructOpt;

#[derive(Debug, Clone, StructOpt)]
enum CliArgs {
    Scrobble {
        /// Artist name
        #[structopt(long)]
        artist: String,

        /// Album name
        #[structopt(long)]
        album: Option<String>,

        /// Track name
        #[structopt(long)]
        track: Option<String>,

        /// Dry run mode (no writes done)
        #[structopt(short, long)]
        dryrun: bool,
    },

    Auth {
        /// API key
        #[structopt(long)]
        api_key: String,

        /// Secret key
        #[structopt(long)]
        secret_key: String,
    },
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
        } if album.is_some() => scrobble_album(artist, album.unwrap(), dryrun),
        CliArgs::Scrobble {
            artist,
            album: _,
            track,
            dryrun,
        } if track.is_some() => scrobble_track(artist, track.unwrap(), dryrun),
        CliArgs::Scrobble { .. } => {
            anyhow::bail!("Wrong arguments");
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
        .init();

    let cli_args = CliArgs::from_args();
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
