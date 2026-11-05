use std::{env, net::IpAddr, path::PathBuf, str::FromStr};

use anyhow::{Context, Result};

pub struct Config {
    pub bind: Option<IpAddr>,
    pub port: u16,
    pub relay_port: u16,
    pub control_port: u16,
    pub control_secret: String,
    pub relay_host: String,
    pub encrypted_only: bool,
    pub always_use_relay: bool,
    pub trust_proxy_headers: bool,
    pub data_dir: PathBuf,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let port = parse_or("PORT", 21116)?;
        let relay_port = parse_or("RELAY_PORT", 21117)?;
        Ok(Self {
            bind: parse_optional("BIND")?,
            port,
            relay_port,
            control_port: parse_or("CONTROL_PORT", relay_port + 3)?,
            control_secret: env::var("CONTROL_SECRET").unwrap_or_default().trim().to_owned(),
            relay_host: env::var("RELAY_HOST").unwrap_or_default().trim().to_owned(),
            encrypted_only: flag("ENCRYPTED_ONLY"),
            always_use_relay: flag("ALWAYS_USE_RELAY"),
            trust_proxy_headers: flag("TRUST_PROXY_HEADERS"),
            data_dir: env::var("DATA_DIR").unwrap_or_else(|_| "data".to_owned()).into(),
        })
    }

    pub fn nat_test_port(&self) -> u16 {
        self.port - 1
    }

    pub fn websocket_port(&self) -> u16 {
        self.port + 2
    }

    pub fn relay_websocket_port(&self) -> u16 {
        self.relay_port + 2
    }
}

fn flag(name: &str) -> bool {
    matches!(
        env::var(name).unwrap_or_default().to_ascii_lowercase().as_str(),
        "y" | "yes" | "1" | "true"
    )
}

fn parse_optional<T: FromStr>(name: &str) -> Result<Option<T>>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .map(Some)
            .with_context(|| format!("invalid value for {name}: {value}")),
        _ => Ok(None),
    }
}

fn parse_or<T: FromStr>(name: &str, default: T) -> Result<T>
where
    T::Err: std::error::Error + Send + Sync + 'static,
{
    Ok(parse_optional(name)?.unwrap_or(default))
}
