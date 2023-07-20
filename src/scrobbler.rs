use crate::auth::load_auth_config;
use crate::lastfmapi::ApiError;
use crate::lastfmapi::LastfmApi;
use log::warn;
use time::OffsetDateTime;

pub fn scrobble_track(artist: String, track: String, _dryrun: bool) -> Result<(), anyhow::Error> {
    let auth_config = load_auth_config()?;
    let api = LastfmApi::new(auth_config);
    let when = OffsetDateTime::now_utc();
    match api.scrobble(artist, track, when) {
        Ok(()) => Ok(()),
        Err(ApiError::Unscrobbled(reason)) => {
            warn!("Not scrobbled due to: {}", reason);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
