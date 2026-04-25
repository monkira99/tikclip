use crate::db::models::FlowNodeDefinition;
use crate::live_runtime::normalize::{canonicalize_username, CanonicalUsername};
use serde::Deserialize;
use serde_json::Value;

use super::EngineNodeResult;

#[derive(Debug, Deserialize)]
struct RawStartConfig {
    username: String,
    #[serde(default, alias = "cookiesJson")]
    cookies_json: String,
    #[serde(default, alias = "proxyUrl")]
    proxy_url: String,
    #[serde(default = "default_waf_bypass_enabled", alias = "wafBypassEnabled")]
    waf_bypass_enabled: bool,
    #[serde(
        default = "default_poll_interval_seconds",
        alias = "pollIntervalSeconds"
    )]
    poll_interval_seconds: i64,
    #[serde(default, alias = "retryLimit")]
    retry_limit: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StartConfig {
    pub username: CanonicalUsername,
    pub cookies_json: String,
    pub proxy_url: Option<String>,
    pub waf_bypass_enabled: bool,
    pub poll_interval_seconds: i64,
    pub retry_limit: i64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, serde::Serialize)]
pub struct StartOutput {
    pub account_id: i64,
    pub username: String,
    pub room_id: String,
    pub stream_url: String,
    pub viewer_count: Option<i64>,
    pub detected_at: String,
}

pub fn parse_start_config(raw: &str) -> Result<StartConfig, String> {
    let cfg: RawStartConfig = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let username =
        canonicalize_username(&cfg.username).map_err(|_| "username is required".to_string())?;
    let proxy_url = cfg.proxy_url.trim();

    Ok(StartConfig {
        username,
        cookies_json: cfg.cookies_json,
        proxy_url: (!proxy_url.is_empty()).then(|| proxy_url.to_string()),
        waf_bypass_enabled: cfg.waf_bypass_enabled,
        poll_interval_seconds: cfg.poll_interval_seconds.max(5),
        retry_limit: cfg.retry_limit.max(0),
    })
}

fn canonicalize_start_config_json_with_mode(
    raw: &str,
    require_username: bool,
) -> Result<String, String> {
    let mut value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "start config must be a JSON object".to_string())?;
    let username = object.get("username").and_then(Value::as_str).unwrap_or("");
    let canonical_username = match canonicalize_username(username) {
        Ok(canonical) => canonical.canonical,
        Err(_) if require_username => return Err("username is required".to_string()),
        Err(_) => String::new(),
    };
    let cookies_json = object
        .get("cookies_json")
        .or_else(|| object.get("cookiesJson"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let proxy_url = object
        .get("proxy_url")
        .or_else(|| object.get("proxyUrl"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let poll_interval_seconds = object
        .get("poll_interval_seconds")
        .or_else(|| object.get("pollIntervalSeconds"))
        .and_then(Value::as_i64)
        .unwrap_or(default_poll_interval_seconds())
        .max(5);
    let waf_bypass_enabled = object
        .get("waf_bypass_enabled")
        .or_else(|| object.get("wafBypassEnabled"))
        .and_then(value_as_bool)
        .unwrap_or_else(default_waf_bypass_enabled);
    let retry_limit = object
        .get("retry_limit")
        .or_else(|| object.get("retryLimit"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);

    object.insert("username".to_string(), Value::String(canonical_username));
    object.insert("cookies_json".to_string(), Value::String(cookies_json));
    object.insert("proxy_url".to_string(), Value::String(proxy_url));
    object.insert(
        "waf_bypass_enabled".to_string(),
        Value::Bool(waf_bypass_enabled),
    );
    object.insert(
        "poll_interval_seconds".to_string(),
        Value::Number(poll_interval_seconds.into()),
    );
    object.insert("retry_limit".to_string(), Value::Number(retry_limit.into()));

    object.remove("cookiesJson");
    object.remove("proxyUrl");
    object.remove("wafBypassEnabled");
    object.remove("pollIntervalSeconds");
    object.remove("retryLimit");

    serde_json::to_string(&value).map_err(|e| e.to_string())
}

pub fn canonicalize_start_config_json(raw: &str) -> Result<String, String> {
    canonicalize_start_config_json_with_mode(raw, true)
}

pub fn canonicalize_start_draft_config_json(raw: &str) -> Result<String, String> {
    canonicalize_start_config_json_with_mode(raw, false)
}

fn default_poll_interval_seconds() -> i64 {
    60
}

fn default_waf_bypass_enabled() -> bool {
    true
}

fn value_as_bool(value: &Value) -> Option<bool> {
    if let Some(flag) = value.as_bool() {
        return Some(flag);
    }
    if let Some(number) = value.as_i64() {
        return Some(number != 0);
    }
    let raw = value.as_str()?.trim().to_ascii_lowercase();
    match raw.as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn run(def: &FlowNodeDefinition, input_json: Option<&str>) -> Result<EngineNodeResult, String> {
    let _ = parse_start_config(def.published_config_json.as_str())?;
    let _ = input_json;
    Ok(EngineNodeResult {
        status: "completed".to_string(),
        output_json: input_json.map(|x| x.to_string()),
        error: None,
        next_node: Some("record".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::{canonicalize_start_config_json, parse_start_config, StartOutput};
    use serde_json::Value;

    #[test]
    fn parse_start_config_reads_snake_case_fields() {
        let cfg = parse_start_config(
            r#"{
                "username":" @shop_abc ",
                "cookies_json":"{}",
                "proxy_url":"http://127.0.0.1:9000",
                "waf_bypass_enabled":false,
                "poll_interval_seconds":20,
                "retry_limit":3
            }"#,
        )
        .unwrap();

        assert_eq!(cfg.username.canonical, "shop_abc");
        assert_eq!(cfg.cookies_json, "{}");
        assert_eq!(cfg.proxy_url.as_deref(), Some("http://127.0.0.1:9000"));
        assert!(!cfg.waf_bypass_enabled);
        assert_eq!(cfg.poll_interval_seconds, 20);
        assert_eq!(cfg.retry_limit, 3);
    }

    #[test]
    fn parse_start_config_accepts_legacy_camel_case_fields() {
        let cfg = parse_start_config(
            r#"{
                "username":" @shop_abc ",
                "cookiesJson":"{}",
                "proxyUrl":"http://127.0.0.1:9000",
                "wafBypassEnabled":false,
                "pollIntervalSeconds":20,
                "retryLimit":3
            }"#,
        )
        .unwrap();

        assert_eq!(cfg.username.canonical, "shop_abc");
        assert_eq!(cfg.cookies_json, "{}");
        assert_eq!(cfg.proxy_url.as_deref(), Some("http://127.0.0.1:9000"));
        assert!(!cfg.waf_bypass_enabled);
        assert_eq!(cfg.poll_interval_seconds, 20);
        assert_eq!(cfg.retry_limit, 3);
    }

    #[test]
    fn canonicalize_start_config_json_preserves_runtime_and_unrelated_keys() {
        let raw = r#"{"username":" @shop_abc ","last_live_at":"live-ts","last_run_at":"run-ts","last_error":"x","custom":"keep-me"}"#;
        let canonical = canonicalize_start_config_json(raw).unwrap();

        let value: Value = serde_json::from_str(&canonical).unwrap();
        assert_eq!(
            value.get("username").and_then(Value::as_str),
            Some("shop_abc")
        );
        assert_eq!(
            value.get("last_live_at").and_then(Value::as_str),
            Some("live-ts")
        );
        assert_eq!(
            value.get("last_run_at").and_then(Value::as_str),
            Some("run-ts")
        );
        assert_eq!(value.get("last_error").and_then(Value::as_str), Some("x"));
        assert_eq!(value.get("custom").and_then(Value::as_str), Some("keep-me"));
        assert_eq!(value.get("cookies_json").and_then(Value::as_str), Some(""));
        assert_eq!(
            value.get("waf_bypass_enabled").and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn start_output_serializes_resolved_live_payload() {
        let output = StartOutput {
            account_id: 9,
            username: "shop_abc".to_string(),
            room_id: "7312345".to_string(),
            stream_url: "https://example.com/live.flv".to_string(),
            viewer_count: Some(77),
            detected_at: "2026-04-18 09:30:00".to_string(),
        };

        let value: Value = serde_json::to_value(output).unwrap();

        assert_eq!(value.get("account_id").and_then(Value::as_i64), Some(9));
        assert_eq!(
            value.get("room_id").and_then(Value::as_str),
            Some("7312345")
        );
        assert_eq!(
            value.get("stream_url").and_then(Value::as_str),
            Some("https://example.com/live.flv")
        );
    }
}
