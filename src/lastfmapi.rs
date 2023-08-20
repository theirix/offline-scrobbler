use log::{debug, error, info};
use reqwest::blocking::Client;
use serde_json::Value;
use std::collections::HashMap;
use time::OffsetDateTime;
use xmltree::Element;

use crate::auth::AuthConfig;

const AUDIOSCROBBLER_HOST: &str = "https://ws.audioscrobbler.com";

/// Last.fm API client
pub struct LastfmApi {
    auth_config: AuthConfig,
    client: Client,
    api_host: String,
}

/// Last.fm API and scrobbling errors
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
    pub url: Option<String>,
}

impl LastfmApi {
    pub fn new(auth_config: AuthConfig, api_host: String) -> Self {
        let client = Client::new();
        Self {
            auth_config,
            client,
            api_host,
        }
    }

    pub fn get_request_token(&self) -> Result<String, ApiError> {
        let url = format!(
            "{api_host}/2.0/?method=auth.gettoken&api_key={key}&format=json",
            api_host = self.api_host,
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
        let url = format!("{}/2.0", self.api_host);
        let response = self
            .client
            .post(url)
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
        let url = format!("{}/2.0", self.api_host);
        let response = self
            .client
            .post(url)
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
            "{api_host}/2.0/\
                ?method=album.getInfo&artist={artist}&album={album}&api_key={key}&format=json",
            api_host = self.api_host,
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

        let jalbum = resp
            .as_object()
            .ok_or(ApiError::Json)?
            .get("album")
            .ok_or(ApiError::Json)?;

        if jalbum.get("tracks").is_none() {
            return Err(ApiError::Unscrobbled("Empty album".into()));
        }

        let jtracks = jalbum
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

        let album_url: Option<String> = resp
            .as_object()
            .ok_or(ApiError::Json)?
            .get("album")
            .ok_or(ApiError::Json)?
            .get("url")
            .ok_or(ApiError::Json)?
            .as_str()
            .map(|s| s.to_string());

        let album_struct = Album {
            title,
            tracks,
            url: album_url,
        };

        Ok(album_struct)
    }

    fn parse_track(&self, jtrack: &Value) -> anyhow::Result<Track, ApiError> {
        let default_duration: i64 = 300;
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
            .unwrap_or(default_duration);
        Ok(Track { duration, title })
    }
}

/// Last.fm API client builder
pub struct LastfmApiBuilder {
    auth_config: AuthConfig,
    api_host: String,
}

#[allow(dead_code)]
impl LastfmApiBuilder {
    pub fn new(auth_config: AuthConfig) -> LastfmApiBuilder {
        LastfmApiBuilder {
            auth_config,
            api_host: AUDIOSCROBBLER_HOST.to_string(),
        }
    }

    pub fn with_api_host(mut self, api_host: String) -> LastfmApiBuilder {
        self.api_host = api_host;
        self
    }

    pub fn build(self) -> LastfmApi {
        LastfmApi::new(self.auth_config, self.api_host)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::lastfmapi::AuthConfig;
    use crate::utils::now_local;
    use httpmock::prelude::*;
    use test_log::test;

    fn mock_client(server: &MockServer) -> LastfmApi {
        let api_host = "http://".to_owned() + &server.address().to_string();
        info!("Using mock server address {}", api_host);
        let auth_config = AuthConfig {
            api_key: String::new(),
            secret_key: String::new(),
            session_key: String::new(),
        };
        LastfmApiBuilder::new(auth_config)
            .with_api_host(api_host)
            .build()
    }

    #[test]
    fn test_request_token() {
        let server = MockServer::start();

        let mock_gettoken = server.mock(|when, then| {
            when.method(POST)
                .path("/2.0/")
                .query_param("method", "auth.gettoken");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"token": "secrettoken"}"#);
        });

        let res = mock_client(&server).get_request_token();
        mock_gettoken.assert();
        assert!(res.is_ok());
    }

    #[test]
    fn test_request_token_fail() {
        let server = MockServer::start();

        let mock_gettoken = server.mock(|when, then| {
            when.method(POST)
                .path("/2.0/")
                .query_param("method", "auth.gettoken");
            then.status(400)
                .header("content-type", "application/json")
                .body(r#"{}"#);
        });

        let res = mock_client(&server).get_request_token();
        mock_gettoken.assert();
        assert!(res.is_err());
        assert!(matches!(res.unwrap_err(), ApiError::Generic(_)));
    }

    #[test]
    fn test_get_album_tracks() {
        let server = MockServer::start();

        let response_text = include_str!("data/resp.album.json");
        let mock_gettoken = server.mock(|when, then| {
            when.method(POST)
                .path("/2.0/")
                .query_param("method", "album.getInfo");
            then.status(200)
                .header("content-type", "application/json")
                .body(response_text);
        });

        let res = mock_client(&server).get_album_tracks(
            "Hooverphonic".into(),
            "A New Stereophonic Sound Spectacular".into(),
        );
        mock_gettoken.assert();
        assert!(res.is_ok());
        let album = res.unwrap();
        assert_eq!(album.title, "A New Stereophonic Sound Spectacular");
        assert_eq!(
            album.url.unwrap_or("".into()),
            "https://www.last.fm/music/Hooverphonic/A+New+Stereophonic+Sound+Spectacular"
        );
        assert_eq!(album.tracks.len(), 11);
    }

    #[test]
    fn test_scrobble() {
        let server = MockServer::start();

        let response_text = include_str!("data/resp.scrobble.json");
        let mock_gettoken = server.mock(|when, then| {
            when.method(POST)
                .path("/2.0")
                .x_www_form_urlencoded_tuple("method", "track.scrobble");
            then.status(200)
                .header("content-type", "application/json")
                .body(response_text);
        });

        let res = mock_client(&server).scrobble("Hooverphonic".into(), "Eden".into(), now_local());
        mock_gettoken.assert();
        assert!(res.is_ok());
    }
}
