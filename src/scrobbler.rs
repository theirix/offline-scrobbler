use crate::auth::load_auth_config;
use crate::lastfmapi::{Album, ApiError, LastfmApi, LastfmApiBuilder};
use crate::utils::now_local;
use anyhow::{anyhow, Context};
use log::{debug, info, warn};
use time::ext::NumericalDuration;
use time::macros::format_description;
use time::Duration;
use url::Url;

/// Scrobble all tracks in an album with proper timestamps
fn scrobble_timeline(
    api: &LastfmApi,
    artist: &String,
    album: Album,
    dryrun: bool,
    offset: Duration,
) -> Result<(), anyhow::Error> {
    let now = now_local();
    let album_len: i64 = album.tracks.iter().map(|track| track.duration).sum();
    let track_gap = 5.seconds();

    let mut start_time =
        now - Duration::new(album_len, 0) - ((album.tracks.len() - 1) as i16) * track_gap - offset;
    let mut any_unscrobbled = false;
    for idx in 0..album.tracks.len() {
        let track = &album.tracks[idx];
        start_time += Duration::new(track.duration, 0) + track_gap;
        info!(
            "{} track #{} '{}' of artist '{}' at {}",
            if dryrun { "Previewing" } else { "Scrobbling" },
            idx + 1,
            &track.title,
            &artist,
            start_time.format(format_description!("[hour]:[minute]:[second]"))?,
        );
        if !dryrun {
            match api.scrobble(artist.clone(), track.title.clone(), start_time) {
                Ok(_) => {}
                Err(ApiError::Unscrobbled(reason)) => {
                    warn!("Not scrobbled due to: {}", reason);
                    any_unscrobbled = true;
                }
                Err(e) => return Err(e.into()),
            };
        }
    }

    if any_unscrobbled {
        Err(anyhow!(format!("Not all tracks scrobbled")))
    } else {
        Ok(())
    }
}

/// Scrobble a whole album of an artist
pub fn scrobble_album(
    artist: String,
    album: String,
    dryrun: bool,
    start: Option<Duration>,
) -> Result<(), anyhow::Error> {
    let auth_config = load_auth_config()?;
    let api = LastfmApiBuilder::new(auth_config).build();
    // When the track scrobbled - subset offset from current time
    let offset = start.map_or(Duration::ZERO, |v| v);
    debug!("Scrobble offset {:?}", offset);

    match api.get_album_tracks(artist.clone(), album.clone()) {
        Ok(album_info) => {
            if album_info.title != album {
                warn!(
                    "Album name {} differs from given {}",
                    &album_info.title, &album
                );
            }
            info!("Album name {}", &album_info.title);
            if let Some(album_url) = &album_info.url {
                info!("Album url {}", &album_url);
            }
            scrobble_timeline(&api, &artist, album_info, dryrun, offset)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Scrobble a track of an artist
pub fn scrobble_track(
    artist: String,
    track: String,
    _dryrun: bool,
    start: Option<Duration>,
) -> Result<(), anyhow::Error> {
    let auth_config = load_auth_config()?;
    let api = LastfmApiBuilder::new(auth_config).build();
    // When the track scrobbled - subset offset from current time
    let offset = start.map_or(Duration::ZERO, |v| v);
    let when = now_local() - offset;
    match api.scrobble(artist, track, when) {
        Ok(()) => Ok(()),
        Err(ApiError::Unscrobbled(reason)) => {
            warn!("Not scrobbled due to: {}", reason);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Scrobble a whole album identified by Last.fm webpage URL
pub fn scrobble_url(
    url: String,
    dryrun: bool,
    start: Option<Duration>,
) -> Result<(), anyhow::Error> {
    let expected_format = "https://www.last.fm/music/Artist/Album+Name";

    let parsed_url = Url::parse(&url)?;
    debug!("Parsed url to: {:?}", &parsed_url);

    let path = &parsed_url
        .path_segments()
        .context("Cannot parse path")?
        .collect::<Vec<&str>>();
    if !(parsed_url.host_str() == Some("last.fm") || parsed_url.host_str() == Some("www.last.fm")) {
        anyhow::bail!("URL is not from last.fm");
    }
    if path.len() < 3 || path[0] != "music" {
        anyhow::bail!("URL must be in format {}", expected_format);
    }
    // Additionally replace plus (not %20) with space
    let artist = urlencoding::decode(path[1])?.replace('+', " ");
    let album = urlencoding::decode(path[2])?.replace('+', " ");

    info!("Extracted artist {} and album {}", &artist, &album);

    scrobble_album(artist, album, dryrun, start)
}
