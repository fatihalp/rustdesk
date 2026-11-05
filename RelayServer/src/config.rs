use std::{env, net::IpAddr, path::PathBuf, str::FromStr};

use anyhow::{Context, Result};

/// Runtime settings, read once from environment variables.
pub struct Config {
    /// Address to bind to; all interfaces when unset.
    pub bind: Option<IpAddr>,
    /// Rendezvous port (TCP and UDP). The NAT-test port is `port - 1`, the WebSocket port `port + 2`.
    pub port: u16,
    /// Relay port (TCP). The relay WebSocket port is `relay_port + 2`.
    pub relay_port: u16,
    /// Control & real-time WebSocket port for the Laravel management panel (`relay_port + 3` by default).
    pub control_port: u16,
    /// Optional shared secret required on control mutations (`X-Relay-Secret` header).
    pub control_secret: String,
    /// Relay address advertised to clients; empty lets clients use their own setting.
    pub relay_host: String,
    /// Only accept clients that present this server's public key.
    pub encrypted_only: bool,
    /// Skip NAT punching and force every session through the relay.
    pub always_use_relay: bool,
    /// Trust `X-Real-IP` / `X-Forwarded-For` on WebSocket ports (only behind a reverse proxy).
    pub trust_proxy_headers: bool,
    /// Directory holding the signing key pair and the peer database.
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
