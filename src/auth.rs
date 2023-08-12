use crate::lastfmapi::LastfmApiBuilder;
use anyhow::Context;
use directories::ProjectDirs;
use log::info;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub api_key: String,
    pub secret_key: String,
    pub session_key: String,
}

//pub fn is_authenticated() -> anyhow::Result<bool> {
//Ok(config_file()?.is_file())
//}

/// Provide path to auth config file
fn config_file() -> anyhow::Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("ru", "omniverse", "offline-scrobbler")
        .context("cannot detect config dir")?;
    let config_path = proj_dirs.config_dir();
    let config_file = config_path.join("config.toml");
    std::fs::create_dir_all(config_path)?;
    info!("Using auth config file {}", config_file.display());
    Ok(config_file.to_path_buf())
}

fn save_auth_config(
    api_key: String,
    secret_key: String,
    session_key: String,
) -> anyhow::Result<()> {
    let config = AuthConfig {
        api_key,
        secret_key,
        session_key,
    };
    let serialized: String = toml::to_string(&config)?;

    fs::write(
        config_file().context("cannot find config file")?,
        serialized,
    )?;
    Ok(())
}

pub fn load_auth_config() -> anyhow::Result<AuthConfig> {
    let serialized = fs::read_to_string(config_file().context("cannot find config file")?)?;
    let config: AuthConfig = toml::from_str(&serialized)?;

    Ok(config)
}

pub fn authenticate(api_key: String, secret_key: String) -> anyhow::Result<()> {
    let auth_config = AuthConfig {
        api_key: api_key.clone(),
        secret_key: secret_key.clone(),
        session_key: "".into(),
    };
    let api = LastfmApiBuilder::new(auth_config).build();

    let request_token = api.get_request_token()?;

    let url = format!(
        "http://www.last.fm/api/auth/?api_key={key}&token={request_token}",
        key = api_key,
        request_token = request_token
    );
    info!("Please open the URL\n{}\nand confirm permission", url);
    info!("Press any key to continue...");

    let mut dummy = String::new();
    if std::io::stdin().read_line(&mut dummy).is_ok() {
        info!("Waiting done");
    }

    let token = api
        .get_session_token(request_token)
        .context("cannot get session token")?;
    info!("Got token {}", &token);
    save_auth_config(api_key, secret_key, token)?;
    Ok(())
}
