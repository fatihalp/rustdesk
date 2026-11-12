use hbb_common::regex::Regex;
use std::ops::Deref;

mod en;

pub const LANGS: &[(&str, &str)] = &[
    ("en", "English"),
];

pub(crate) fn cjk_ui_unavailable() -> bool {
    false
}

pub(crate) fn is_cjk_lang(_lang_or_locale: &str) -> bool {
    false
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn translate(name: String) -> String {
    let locale = sys_locale::get_locale().unwrap_or_default();
    translate_locale(name, &locale)
}

pub fn translate_locale(name: String, _locale: &str) -> String {
    let m = en::T.deref();
    let (name, placeholder_value) = extract_placeholder(&name);
    let replace = |s: &&str| {
        let mut s = s.to_string();
        if let Some(value) = placeholder_value.as_ref() {
            s = s.replace("{}", &value);
        }
        if !crate::is_supportdesk() {
            if s.contains("SupportDesk")
                && !name.starts_with("upgrade_supportdesk_server_pro")
            {
                let app_name = crate::get_app_name();
                if !app_name.contains("SupportDesk") {
                    s = s.replace("SupportDesk", &app_name);
                } else {
                    const PLACEHOLDER: &str = "#A-P-P-N-A-M-E#";
                    if !s.contains(PLACEHOLDER) {
                        s = s.replace(&app_name, PLACEHOLDER);
                        s = s.replace("SupportDesk", &app_name);
                        s = s.replace(PLACEHOLDER, &app_name);
                    }
                }
            }
        }
        s
    };
    if let Some(v) = m.get(&name as &str) {
        if !v.is_empty() {
            return replace(v);
        }
    }
    replace(&name.as_str())
}

fn extract_placeholder(input: &str) -> (String, Option<String>) {
    if let Ok(re) = Regex::new(r#"\{(.*?)\}"#) {
        if let Some(captures) = re.captures(input) {
            if let Some(inner_match) = captures.get(1) {
                let name = re.replace(input, "{}").to_string();
                let value = inner_match.as_str().to_string();
                return (name, Some(value));
            }
        }
    }
    (input.to_string(), None)
}
