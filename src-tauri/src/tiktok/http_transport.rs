use reqwest::header::{
    HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, ORIGIN, REFERER, USER_AGENT,
};
use serde_json::Value;
use std::collections::BTreeSet;
use std::time::Duration;
use thiserror::Error;

#[cfg_attr(not(test), allow(dead_code))]
pub fn normalize_cookie_header(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }

    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "cookies_json must be a JSON object".to_string())?;

    let has_sessionid = object
        .get("sessionid")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty());

    let mut pairs = object
        .iter()
        .filter_map(|(key, value)| {
            value.as_str().and_then(|value| {
                if key == "sessionid" && !has_sessionid {
                    None
                } else {
                    Some(format!("{key}={value}"))
                }
            })
        })
        .collect::<Vec<_>>();

    if !has_sessionid {
        if let Some(sessionid_ss) = object
            .get("sessionid_ss")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        {
            pairs.push(format!("sessionid={sessionid_ss}"));
        }
    }

    Ok(pairs.join("; "))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn cookie_key_summary(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok("no cookies".to_string());
    }

    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "cookies_json must be a JSON object".to_string())?;

    if object.is_empty() {
        return Ok("no cookies".to_string());
    }

    let mut keys = BTreeSet::new();
    for key in object.keys() {
        keys.insert(key.to_string());
    }
    let has_sessionid = object
        .get("sessionid")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty());
    if !has_sessionid
        && object
            .get("sessionid_ss")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
    {
        keys.insert("sessionid".to_string());
    }
    Ok(keys.into_iter().collect::<Vec<_>>().join(","))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn proxy_url_for_reqwest(raw: Option<&str>) -> Result<Option<String>, String> {
    let trimmed = raw.unwrap_or_default().trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err("proxy_url must start with http:// or https://".to_string());
    }

    reqwest::Proxy::all(trimmed).map_err(|e| e.to_string())?;
    Ok(Some(trimmed.to_string()))
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("HTTP {status_code} {url}")]
pub struct TikTokHttpStatusError {
    pub status_code: u16,
    pub url: String,
    pub text: String,
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn ensure_success_status(
    status_code: u16,
    url: &str,
    text: &str,
) -> Result<(), TikTokHttpStatusError> {
    if (200..300).contains(&status_code) {
        return Ok(());
    }
    Err(TikTokHttpStatusError {
        status_code,
        url: url.to_string(),
        text: text.to_string(),
    })
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn default_tiktok_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        ACCEPT,
        HeaderValue::from_static(
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,application/json,text/plain,*/*;q=0.8",
        ),
    );
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.tiktok.com"));
    headers.insert(REFERER, HeaderValue::from_static("https://www.tiktok.com/"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        ),
    );
    headers
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn build_tiktok_reqwest_client(
    cookie_header: &str,
    proxy_url: Option<&str>,
    timeout: Duration,
) -> Result<reqwest::Client, String> {
    let mut headers = default_tiktok_headers();
    if !cookie_header.trim().is_empty() {
        headers.insert(
            reqwest::header::COOKIE,
            HeaderValue::from_str(cookie_header).map_err(|e| e.to_string())?,
        );
    }

    let mut builder = reqwest::Client::builder()
        .default_headers(headers)
        .timeout(timeout)
        .redirect(reqwest::redirect::Policy::limited(10));

    if let Some(proxy) = proxy_url_for_reqwest(proxy_url)? {
        builder = builder.proxy(reqwest::Proxy::all(proxy).map_err(|e| e.to_string())?);
    }

    builder.build().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        build_tiktok_reqwest_client, cookie_key_summary, default_tiktok_headers,
        ensure_success_status, normalize_cookie_header, proxy_url_for_reqwest,
    };
    use reqwest::header::{ACCEPT, ORIGIN, REFERER, USER_AGENT};
    use std::time::Duration;

    #[test]
    fn normalize_cookie_header_adds_sessionid_from_sessionid_ss_when_missing() {
        let cookies =
            normalize_cookie_header(r#"{"sessionid_ss":"abc","tt-target-idc":"useast2a"}"#)
                .unwrap();

        assert_eq!(
            cookies,
            "sessionid_ss=abc; tt-target-idc=useast2a; sessionid=abc"
        );
    }

    #[test]
    fn normalize_cookie_header_keeps_existing_sessionid() {
        let cookies =
            normalize_cookie_header(r#"{"sessionid":"root","sessionid_ss":"leaf"}"#).unwrap();

        assert_eq!(cookies, "sessionid=root; sessionid_ss=leaf");
    }

    #[test]
    fn normalize_cookie_header_treats_empty_sessionid_as_missing_without_duplicates() {
        let cookies = normalize_cookie_header(
            r#"{"sessionid":"","sessionid_ss":"abc","tt-target-idc":"useast2a"}"#,
        )
        .unwrap();

        assert_eq!(
            cookies,
            "sessionid_ss=abc; tt-target-idc=useast2a; sessionid=abc"
        );
    }

    #[test]
    fn cookie_key_summary_reports_only_sorted_keys() {
        let summary = cookie_key_summary(r#"{"tt-target-idc":"x","sessionid_ss":"abc"}"#).unwrap();

        assert_eq!(summary, "sessionid,sessionid_ss,tt-target-idc");
    }

    #[test]
    fn proxy_url_for_reqwest_rejects_non_http_schemes() {
        let err = proxy_url_for_reqwest(Some("socks5://127.0.0.1:9000")).unwrap_err();

        assert!(err.contains("http:// or https://"));
    }

    #[test]
    fn ensure_success_status_returns_typed_error_with_status_url_and_text() {
        let err =
            ensure_success_status(429, "https://www.tiktok.com/api/x", "rate limited").unwrap_err();

        assert_eq!(err.status_code, 429);
        assert_eq!(err.url, "https://www.tiktok.com/api/x");
        assert_eq!(err.text, "rate limited");
        assert_eq!(err.to_string(), "HTTP 429 https://www.tiktok.com/api/x");
    }

    #[test]
    fn default_tiktok_headers_match_expected_browser_navigation_shape() {
        let headers = default_tiktok_headers();

        assert_eq!(
            headers.get(ACCEPT).and_then(|v| v.to_str().ok()),
            Some(
                "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,application/json,text/plain,*/*;q=0.8"
            )
        );
        assert_eq!(
            headers.get(ORIGIN).and_then(|v| v.to_str().ok()),
            Some("https://www.tiktok.com")
        );
        assert_eq!(
            headers.get(REFERER).and_then(|v| v.to_str().ok()),
            Some("https://www.tiktok.com/")
        );
        assert_eq!(
            headers.get(USER_AGENT).and_then(|v| v.to_str().ok()),
            Some(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            )
        );
    }

    #[test]
    fn build_tiktok_reqwest_client_rejects_invalid_proxy_url() {
        let err = build_tiktok_reqwest_client(
            "sessionid=abc",
            Some("socks5://127.0.0.1:9000"),
            Duration::from_secs(8),
        )
        .unwrap_err();

        assert!(err.contains("http:// or https://"));
    }
}
