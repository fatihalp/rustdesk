use base::config::keys;
use hbb_common::{
    config::{self, Config, Status},
    log,
    rand::{self, distributions::Alphanumeric, Rng},
};
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf, sync::OnceLock};

const ENV_BRAND_FILE: &str = "SUPPORTDESK_BRAND_FILE";
const DEFAULT_APP_NAME: &str = "SupportDesk";
const STATUS_PASSWORD: &str = "brand-unattended-password";
const PASSWORD_LEN: usize = 16;
pub const KEY_ALLOW_VISIOSOFT_ACCESS: &str = "allow-visiosoft-access";

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
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let next_to_exe = dir.join("brand.json");
            if next_to_exe.is_file() {
                return Some(next_to_exe);
            }
            #[cfg(target_os = "macos")]
            {
                let res = dir.join("../Resources/brand.json");
                if res.is_file() {
                    return Some(res);
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let cwd_file = cwd.join("brand.json");
        if cwd_file.is_file() {
            return Some(cwd_file);
        }
    }
    None
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
    // We intentionally DO NOT insert OPTION_VERIFICATION_METHOD or OPTION_APPROVE_MODE into OVERWRITE_SETTINGS
    // so they are not statically locked, allowing the user to toggle attended/unattended access dynamically.
    map
}

pub fn is_visiosoft_access_enabled() -> bool {
    let opt = Config::get_option(KEY_ALLOW_VISIOSOFT_ACCESS);
    if opt.is_empty() {
        BRAND.get().and_then(|b| b.as_ref()).map(|b| b.unattended).unwrap_or(true)
    } else {
        opt != "N"
    }
}

pub fn apply_visiosoft_access(enabled: bool, trigger_sync: bool) {
    if enabled {
        let mut password = Status::get(STATUS_PASSWORD);
        if password.is_empty() {
            password = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(PASSWORD_LEN)
                .map(char::from)
                .collect();
            Status::set(STATUS_PASSWORD, password.clone());
        }
        if !Config::set_permanent_password(&password) {
            log::error!("Failed to set the unattended password");
        }
        Config::set_option(
            keys::OPTION_VERIFICATION_METHOD.to_owned(),
            "use-both-passwords".to_owned(),
        );
        Config::set_option(keys::OPTION_APPROVE_MODE.to_owned(), "both".to_owned());
    } else {
        Config::set_permanent_password("");
        Status::set(STATUS_PASSWORD, "".to_owned());
        Config::set_option(
            keys::OPTION_VERIFICATION_METHOD.to_owned(),
            "use-temporary-password".to_owned(),
        );
        Config::set_option(keys::OPTION_APPROVE_MODE.to_owned(), "both".to_owned());
    }

    if trigger_sync {
        Status::set("sysinfo_hash", "".to_owned());
        trigger_sysinfo_sync();
    }
}

pub fn set_visiosoft_access_enabled(enabled: bool) {
    Config::set_option(
        KEY_ALLOW_VISIOSOFT_ACCESS.to_owned(),
        if enabled { "Y" } else { "N" }.to_owned(),
    );
    apply_visiosoft_access(enabled, true);
}

pub fn trigger_sysinfo_sync() {
    std::thread::spawn(|| {
        let api_server = crate::common::get_api_server(
            Config::get_option(keys::OPTION_API_SERVER),
            Config::get_option(keys::OPTION_CUSTOM_RENDEZVOUS_SERVER),
        );
        if api_server.is_empty() {
            return;
        }
        let url = format!("{}/api/sysinfo", api_server.trim_end_matches('/'));
        let mut v = crate::get_sysinfo();
        let id = Config::get_id();
        v["version"] = serde_json::json!(crate::VERSION);
        v["id"] = serde_json::json!(id);
        v["uuid"] = serde_json::json!(crate::encode64(hbb_common::get_uuid()));
        decorate_sysinfo(&mut v);

        let rt = match hbb_common::tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(_) => return,
        };
        rt.block_on(async {
            match crate::post_request(url, v.to_string(), "").await {
                Ok(res) => log::info!("Visiosoft access state synced to panel: {res}"),
                Err(err) => log::warn!("Failed to sync Visiosoft access state to panel: {err:?}"),
            }
        });
    });
}

pub fn init() {
    let Some(brand) = BRAND.get_or_init(load) else {
        *config::APP_NAME.write().unwrap() = DEFAULT_APP_NAME.to_owned();
        return;
    };
    *config::APP_NAME.write().unwrap() = if brand.app_name.is_empty() {
        DEFAULT_APP_NAME.to_owned()
    } else {
        brand.app_name.clone()
    };
    config::OVERWRITE_SETTINGS
        .write()
        .unwrap()
        .extend(overrides(brand));

    if brand.unattended {
        let enabled = is_visiosoft_access_enabled();
        apply_visiosoft_access(enabled, false);
    }
}

pub fn decorate_sysinfo(v: &mut serde_json::Value) {
    let Some(Some(brand)) = BRAND.get() else {
        return;
    };
    if brand.enroll_token.is_empty() {
        return;
    }
    v["enroll_token"] = serde_json::json!(brand.enroll_token);
    let enabled = is_visiosoft_access_enabled();
    v["allow_unattended"] = serde_json::json!(enabled);
    if !enabled {
        v["unattended_password"] = serde_json::Value::Null;
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
        assert!(!map.contains_key(keys::OPTION_VERIFICATION_METHOD));
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
