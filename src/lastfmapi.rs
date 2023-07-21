use log::{debug, error, info};
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashMap;
use time::OffsetDateTime;
use xmltree::Element;

use crate::auth::AuthConfig;

const AUDIOSCROBBLER_ROOT: &str = "https://ws.audioscrobbler.com/2.0";

pub struct LastfmApi {
    auth_config: AuthConfig,
    client: Client,
}

#[derive(thiserror::Error, Debug)]
pub enum ApiError {
    #[error("generic: {0}")]
    Generic(String),
    #[error("json error")]
    Json,
    #[error("parse error: {0}")]
    Parse(String),
    #[error("unscrobbled: {0}")]
    Unscrobbled(String),
}

#[derive(Debug)]
pub struct Track {
    pub title: String,
    pub duration: i64,
}

#[derive(Debug)]
pub struct Album {
    pub title: String,
    pub tracks: Vec<Track>,
}

impl LastfmApi {
    pub fn new(auth_config: AuthConfig) -> Self {
        let client = Client::new();
        Self {
            auth_config,
            client,
        }
    }

    pub fn get_request_token(&self) -> Result<String, ApiError> {
        let url = format!(
            "{api_root}/?method=auth.gettoken&api_key={key}&format=json",
            api_root = AUDIOSCROBBLER_ROOT,
            key = self.auth_config.api_key
        );
        let response = self
            .client
            .post(url)
            .body("")
            .send()
            .map_err(|e| ApiError::Generic(e.to_string()))?;

        if !response.status().is_success() {
            error!(
                "Error response to auth.gettoken: {}",
                response.text().unwrap_or("".to_string())
            );
            return Err(ApiError::Generic("Unsuccessfull request".into()));
        }
        let resp: serde_json::Value = response.json().unwrap();
        debug!("Resp {}", resp);
        let token = resp
            .as_object()
            .and_then(|o| o.get("token"))
            .ok_or(ApiError::Json)?
            .as_str()
            .ok_or(ApiError::Json)?
            .to_string();
        info!("Found token {}", &token);
        Ok(token)
    }

    fn compute_signature(&self, params: &HashMap<&str, String>) -> String {
        let mut buf = String::new();
        let mut keys: Vec<&str> = params.keys().copied().collect();
        keys.sort();
        for key in keys {
            buf.push_str(key);
            buf.push_str(params.get(key).unwrap());
        }
        buf.push_str(&self.auth_config.secret_key);

        let md5_hex = format!("{:x}", md5::compute(buf.as_bytes()));

        md5_hex
    }

    pub fn get_session_token(&self, request_token: String) -> Result<String, ApiError> {
        // Build params and signature
        let mut post_params: HashMap<&str, String> = HashMap::from([
            ("api_key", self.auth_config.api_key.clone()),
            ("method", "auth.getSession".to_string()),
            ("token", request_token),
        ]);
        let api_sig = self.compute_signature(&post_params);
        post_params.insert("api_sig", api_sig);

        // Make a request
        let response = self
            .client
            .post(AUDIOSCROBBLER_ROOT)
            .form(&post_params)
            .send()
            .map_err(|e| ApiError::Generic(e.to_string()))?;

        let success = response.status().is_success();
        let response_text = response.text().unwrap_or(String::new());
        if !success {
            error!("Error response to auth.getSession: {}", response_text);
            return Err(ApiError::Generic("Unsuccessfull request".into()));
        }
        debug!("Response: {}", response_text);
        let session_token: String = Element::parse(response_text.as_bytes())
            .map_err(|e| ApiError::Parse(e.to_string()))?
            .get_child("session")
            .ok_or(ApiError::Parse("xml tag session".into()))?
            .get_child("key")
            .ok_or(ApiError::Parse("xml tag key".into()))?
            .get_text()
            .ok_or(ApiError::Parse("xml text".into()))?
            .into_owned();
        Ok(session_token)
    }

    pub fn scrobble(
        &self,
        artist: String,
        track: String,
        when: OffsetDateTime,
    ) -> Result<(), ApiError> {
        // Build params and signature
        let timestamp_sec: i64 = when.unix_timestamp();
        let mut post_params: HashMap<&str, String> = HashMap::from([
            ("api_key", self.auth_config.api_key.clone()),
            ("method", "track.scrobble".to_string()),
            ("artist", artist),
            ("track", track),
            ("timestamp", timestamp_sec.to_string()),
            ("sk", self.auth_config.session_key.clone()),
        ]);
        let api_sig = self.compute_signature(&post_params);
        post_params.insert("api_sig", api_sig);

        // Make a request
        let response = self
            .client
            .post(AUDIOSCROBBLER_ROOT)
            .form(&post_params)
            .send()
            .map_err(|e| ApiError::Generic(e.to_string()))?;

        let success = response.status().is_success();
        let response_text = response.text().unwrap_or(String::new());
        if !success {
            error!("Error response to track.scrobble: {}", response_text);
            return Err(ApiError::Generic("Unsuccessfull request".into()));
        }
        self.parse_scrobble_response(response_text)
    }

    fn parse_scrobble_response(&self, response_text: String) -> anyhow::Result<(), ApiError> {
        debug!("Scrobble response: {}", response_text);
        let elem_root =
            Element::parse(response_text.as_bytes()).map_err(|e| ApiError::Parse(e.to_string()))?;
        let elem_scrobbles = elem_root
            .get_child("scrobbles")
            .ok_or(ApiError::Parse("xml scrobbles key".into()))?;

        let accepted_count: i64 = elem_scrobbles
            .attributes
            .get("accepted")
            .ok_or(ApiError::Parse("no acccepted attr".into()))?
            .parse()
            .map_err(|_| ApiError::Parse("integer".into()))?;
        let ignored_count: i64 = elem_scrobbles
            .attributes
            .get("ignored")
            .ok_or(ApiError::Parse("no ignored attr".into()))?
            .parse()
            .map_err(|_| ApiError::Parse("integer".into()))?;
        if accepted_count == 1 && ignored_count == 0 {
            // It's ok
            Ok(())
        } else if accepted_count == 0 && ignored_count == 1 {
            // Find a reason
            let elem_message = elem_scrobbles
                .get_child("scrobble")
                .ok_or(ApiError::Parse("xml tag scrobble".into()))?
                .get_child("ignoredMessage")
                .ok_or(ApiError::Parse("xml tag ignoredMessage".into()))?;
            let reason_code = elem_message.attributes.get("code").unwrap();
            let reason_text = elem_message
                .get_text()
                .map_or(String::new(), |r| r.into_owned());
            let reason = format!("{}: {}", reason_code, reason_text);
            Err(ApiError::Unscrobbled(reason))
        } else {
            // Invalid structure
            Err(ApiError::Parse("Wrong response structure".into()))
        }
    }

    pub fn get_album_tracks(&self, artist: String, album: String) -> Result<Album, ApiError> {
        let url = format!(
            "https://ws.audioscrobbler.com/2.0/\
                ?method=album.getInfo&artist={artist}&album={album}&api_key={key}&format=json",
            artist = urlencoding::encode(&artist),
            album = urlencoding::encode(&album),
            key = self.auth_config.api_key
        );
        let response = self
            .client
            .post(url)
            .body("")
            .send()
            .map_err(|e| ApiError::Generic(e.to_string()))?;

        if !response.status().is_success() {
            error!("Response: {}", response.text().unwrap_or("".to_string()));
            return Err(ApiError::Generic("Unsuccessfull request".into()));
        }
        let resp: serde_json::Value = response.json().unwrap();
        debug!("Resp {}", resp);

        let jtracks = resp
            .as_object()
            .ok_or(ApiError::Json)?
            .get("album")
            .ok_or(ApiError::Json)?
            .get("tracks")
            .ok_or(ApiError::Json)?
            .get("track")
            .ok_or(ApiError::Json)?
            .as_array()
            .ok_or(ApiError::Json)?;

        debug!("Found {} tracks", jtracks.len());

        let tracks: Vec<Track> = jtracks
            .iter()
            .map(|jtrack| self.parse_track(jtrack))
            .collect::<Result<Vec<Track>, ApiError>>()?;

        let title = resp
            .as_object()
            .ok_or(ApiError::Json)?
            .get("album")
            .ok_or(ApiError::Json)?
            .get("name")
            .ok_or(ApiError::Json)?
            .as_str()
            .ok_or(ApiError::Json)?
            .to_string();

        debug!("Corrected album name {}", &title);

        let album_struct = Album { title, tracks };

        Ok(album_struct)
    }

    fn parse_track(&self, jtrack: &Value) -> anyhow::Result<Track, ApiError> {
        let title = jtrack
            .get("name")
            .ok_or(ApiError::Json)?
            .as_str()
            .ok_or(ApiError::Json)?
            .to_string();
        let duration = jtrack
            .get("duration")
            .ok_or(ApiError::Json)?
            .as_i64()
            .ok_or(ApiError::Json)?;
        Ok(Track { duration, title })
    }
}
