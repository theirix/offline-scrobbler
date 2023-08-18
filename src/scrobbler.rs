use crate::auth::load_auth_config;
use crate::lastfmapi::{Album, ApiError, LastfmApi, LastfmApiBuilder};
use crate::utils::now_local;
use anyhow::anyhow;
use log::{debug, info, warn};
use time::ext::NumericalDuration;
use time::macros::format_description;
use time::Duration;

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
            if dryrun { "Scrobbling" } else { "Previewing" },
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
