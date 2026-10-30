use base::config::keys;
use hbb_common::{
    config::{self, Config, Status},
    log,
    rand::{self, distributions::Alphanumeric, Rng},
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, sync::OnceLock};

const ENV_BRAND_FILE: &str = "RUSTDESK_BRAND_FILE";
const STATUS_PASSWORD: &str = "brand-unattended-password";
const PASSWORD_LEN: usize = 16;

/// Contents of the `brand.json` the support panel packs next to the executable
/// (`Contents/Resources` on macOS) when it builds a customer client.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Brand {
    app_name: String,
    rendezvous_server: String,
    relay_server: String,
    key: String,
    api_server: String,
    enroll_token: String,
    unattended: bool,
}

static BRAND: OnceLock<Option<Brand>> = OnceLock::new();

fn brand_file() -> Option<PathBuf> {
    if let Ok(path) = std::env::var(ENV_BRAND_FILE) {
        return Some(PathBuf::from(path));
    }
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    #[cfg(target_os = "macos")]
    let dir = dir.join("../Resources");
    Some(dir.join("brand.json"))
}

fn load() -> Option<Brand> {
    let path = brand_file().filter(|p| p.is_file())?;
    let data = std::fs::read_to_string(&path)
        .map_err(|e| log::error!("Failed to read brand file: {}", e))
        .ok()?;
    parse(&data)
}

fn parse(data: &str) -> Option<Brand> {
    serde_json::from_str(data.trim_start_matches('\u{feff}'))
        .map_err(|e| log::error!("Failed to parse brand file: {}", e))
        .ok()
}

fn overrides(brand: &Brand) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in [
        (keys::OPTION_CUSTOM_RENDEZVOUS_SERVER, &brand.rendezvous_server),
        (keys::OPTION_RELAY_SERVER, &brand.relay_server),
        (keys::OPTION_KEY, &brand.key),
        (keys::OPTION_API_SERVER, &brand.api_server),
    ] {
        if !value.is_empty() {
            map.insert(key.to_owned(), value.clone());
        }
    }
    if brand.unattended {
        map.insert(
            keys::OPTION_VERIFICATION_METHOD.to_owned(),
            "use-permanent-password".to_owned(),
        );
        map.insert(keys::OPTION_APPROVE_MODE.to_owned(), "password".to_owned());
    }
    map
}

/// Applies the brand file, if any. Call once at startup, before any config is read.
pub fn init() {
    let Some(brand) = BRAND.get_or_init(load) else {
        return;
    };
    if !brand.app_name.is_empty() {
        *config::APP_NAME.write().unwrap() = brand.app_name.clone();
    }
    config::OVERWRITE_SETTINGS
        .write()
        .unwrap()
        .extend(overrides(brand));
}

/// Adds enrollment data to the sysinfo upload so the panel can attach the device
/// to a customer and connect to it without a password prompt.
pub fn decorate_sysinfo(v: &mut serde_json::Value) {
    let Some(Some(brand)) = BRAND.get() else {
        return;
    };
    if brand.enroll_token.is_empty() {
        return;
    }
    v["enroll_token"] = serde_json::json!(brand.enroll_token);
    if !brand.unattended {
        return;
    }
    let mut password = Status::get(STATUS_PASSWORD);
    if password.is_empty() {
        password = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(PASSWORD_LEN)
            .map(char::from)
            .collect();
        if !Config::set_permanent_password(&password) {
            log::error!("Failed to set the unattended password");
            return;
        }
        Status::set(STATUS_PASSWORD, password.clone());
    }
    v["unattended_password"] = serde_json::json!(password);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_panel_manifest_and_builds_overrides() {
        let brand = parse(
            r#"{"app_name":"Acme Support","rendezvous_server":"relay.example.com","key":"K",
                "api_server":"https://panel.example.com","enroll_token":"T","unattended":true}"#,
        )
        .unwrap();
        assert_eq!(brand.app_name, "Acme Support");
        let map = overrides(&brand);
        assert_eq!(map[keys::OPTION_CUSTOM_RENDEZVOUS_SERVER], "relay.example.com");
        assert_eq!(map[keys::OPTION_API_SERVER], "https://panel.example.com");
        assert_eq!(map[keys::OPTION_VERIFICATION_METHOD], "use-permanent-password");
        assert!(!map.contains_key(keys::OPTION_RELAY_SERVER));
    }

    #[test]
    fn attended_brand_keeps_password_prompt_settings() {
        let brand = parse(r#"{"rendezvous_server":"r.example.com"}"#).unwrap();
        let map = overrides(&brand);
        assert!(!map.contains_key(keys::OPTION_VERIFICATION_METHOD));
        assert!(!map.contains_key(keys::OPTION_APPROVE_MODE));
    }
}
