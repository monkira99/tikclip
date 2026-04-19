use serde_json::Value;

use super::types::LiveStatus;

#[cfg_attr(not(test), allow(dead_code))]
const STREAM_QUALITY_ORDER: &[&str] = &["FULL_HD1", "HD1", "SD1", "SD2"];

#[cfg_attr(not(test), allow(dead_code))]
pub fn normalize_cookie_header(raw: &str) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok(String::new());
    }

    let value: Value = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "cookies_json must be a JSON object".to_string())?;

    Ok(object
        .iter()
        .filter_map(|(key, value)| value.as_str().map(|value| format!("{key}={value}")))
        .collect::<Vec<_>>()
        .join("; "))
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

#[cfg_attr(not(test), allow(dead_code))]
pub fn extract_room_id_from_html(html: &str) -> Option<String> {
    const PREFIXES: &[&str] = &[
        "\"roomId\":\"",
        "\"room_id\":\"",
        "\"room_id\":",
        "room_id=",
        "roomId=",
        "\"id_str\":\"",
        "\"web_rid\":\"",
    ];

    for prefix in PREFIXES {
        if let Some(index) = html.find(prefix) {
            let suffix = &html[index + prefix.len()..];
            let digits: String = suffix.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 5 {
                return Some(digits);
            }
        }
    }

    if let Some(index) = html.find("room/") {
        let suffix = &html[index + "room/".len()..];
        let digits: String = suffix.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 10 {
            return Some(digits);
        }
    }

    None
}

fn choose_stream_url(room: &Value) -> Option<String> {
    let stream_url = room.get("stream_url")?;

    for field in ["flv_pull_url", "hls_pull_url_map", "hls_pull_url"] {
        if let Some(map) = stream_url.get(field).and_then(Value::as_object) {
            for quality in STREAM_QUALITY_ORDER {
                if let Some(url) = map.get(*quality).and_then(Value::as_str) {
                    if !url.is_empty() {
                        return Some(url.to_string());
                    }
                }
            }
            if let Some(url) = map.values().find_map(Value::as_str) {
                if !url.is_empty() {
                    return Some(url.to_string());
                }
            }
        }
    }

    for field in ["flv_pull_url", "hls_pull_url"] {
        if let Some(url) = stream_url.get(field).and_then(Value::as_str) {
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }

    None
}

fn live_status_value(room: &Value) -> Option<i64> {
    room.get("LiveRoomInfo")
        .and_then(|value| value.get("status"))
        .and_then(Value::as_i64)
        .or_else(|| room.get("status").and_then(Value::as_i64))
}

fn room_id_value(room: &Value) -> Option<String> {
    for field in ["id_str", "room_id", "web_rid"] {
        if let Some(value) = room.get(field) {
            if let Some(room_id) = value.as_str() {
                if !room_id.is_empty() {
                    return Some(room_id.to_string());
                }
            }

            if let Some(room_id) = value.as_i64() {
                return Some(room_id.to_string());
            }
        }
    }

    None
}

fn viewer_count_value(room: &Value) -> Option<i64> {
    room.get("LiveRoomInfo")
        .and_then(|value| value.get("liveRoomStats"))
        .and_then(|value| value.get("userCount"))
        .and_then(Value::as_i64)
        .or_else(|| room.get("owner_count").and_then(Value::as_i64))
        .or_else(|| room.get("user_count").and_then(Value::as_i64))
        .or_else(|| room.get("viewer_count").and_then(Value::as_i64))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_room_info_live_status(body: &str) -> Result<Option<LiveStatus>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let room = &value["data"]["room"];
    if live_status_value(room).unwrap_or_default() != 2 {
        return Ok(None);
    }

    let room_id = room_id_value(room).unwrap_or_default();
    let stream_url = choose_stream_url(room).ok_or_else(|| "missing stream_url".to_string())?;

    Ok(Some(LiveStatus {
        room_id,
        stream_url: Some(stream_url),
        viewer_count: viewer_count_value(room),
    }))
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_check_alive_live_status(body: &str) -> Result<Option<LiveStatus>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let data = &value["data"];

    let first_row = data
        .as_array()
        .and_then(|rows| rows.first())
        .unwrap_or(data);

    if first_row["alive"].as_i64().unwrap_or_default() != 1 {
        return Ok(None);
    }

    let room_id = first_row["room_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let stream_url = first_row["stream_url"].as_str().map(str::to_string);

    Ok(Some(LiveStatus {
        room_id,
        stream_url,
        viewer_count: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        extract_room_id_from_html, normalize_cookie_header, parse_check_alive_live_status,
        parse_room_info_live_status, proxy_url_for_reqwest,
    };

    #[test]
    fn extract_room_id_from_html_reads_sigil_patterns() {
        let html = r#"...room_id=7312345..."#;

        assert_eq!(extract_room_id_from_html(html), Some("7312345".to_string()));
    }

    #[test]
    fn normalize_cookie_header_accepts_json_cookie_map() {
        let cookies =
            normalize_cookie_header(r#"{"sessionid":"abc","tt-target-idc":"useast2a"}"#).unwrap();

        assert_eq!(cookies, "sessionid=abc; tt-target-idc=useast2a");
    }

    #[test]
    fn normalize_cookie_header_rejects_invalid_json() {
        let err = normalize_cookie_header("not-json").unwrap_err();

        assert!(err.contains("expected"));
    }

    #[test]
    fn parse_room_info_live_status_picks_highest_priority_stream_url_and_viewer_count() {
        let body = r#"{
          "data": {
            "room": {
              "status": 2,
              "id_str": "7312345",
              "owner_count": 321,
              "stream_url": {
                "flv_pull_url": {
                  "SD1":"https://example.com/live-sd.flv",
                  "FULL_HD1":"https://example.com/live-hd.flv"
                }
              }
            }
          }
        }"#;

        let live = parse_room_info_live_status(body).unwrap().unwrap();

        assert_eq!(live.room_id, "7312345");
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/live-hd.flv")
        );
        assert_eq!(live.viewer_count, Some(321));
    }

    #[test]
    fn parse_room_info_live_status_accepts_alternate_room_id_status_and_viewer_shapes() {
        let body = r#"{
          "data": {
            "room": {
              "room_id": "99887766",
              "LiveRoomInfo": {
                "status": 2,
                "liveRoomStats": {
                  "userCount": 456
                }
              },
              "stream_url": {
                "hls_pull_url_map": {
                  "HD1":"https://example.com/live-hls.m3u8"
                }
              }
            }
          }
        }"#;

        let live = parse_room_info_live_status(body).unwrap().unwrap();

        assert_eq!(live.room_id, "99887766");
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/live-hls.m3u8")
        );
        assert_eq!(live.viewer_count, Some(456));
    }

    #[test]
    fn parse_room_info_live_status_accepts_flat_viewer_count_and_web_rid_room_id() {
        let body = r#"{
          "data": {
            "room": {
              "status": 2,
              "web_rid": "11223344",
              "viewer_count": 789,
              "stream_url": {
                "flv_pull_url": "https://example.com/live.flv"
              }
            }
          }
        }"#;

        let live = parse_room_info_live_status(body).unwrap().unwrap();

        assert_eq!(live.room_id, "11223344");
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/live.flv")
        );
        assert_eq!(live.viewer_count, Some(789));
    }

    #[test]
    fn parse_check_alive_live_status_succeeds_without_stream_url_when_live() {
        let body = r#"{"data":{"alive":1,"room_id":"7312345"}}"#;

        let live = parse_check_alive_live_status(body).unwrap().unwrap();

        assert_eq!(live.room_id, "7312345");
        assert_eq!(live.stream_url, None);
    }

    #[test]
    fn parse_check_alive_live_status_keeps_stream_url_when_present() {
        let body = r#"{"data":{"alive":1,"room_id":"7312345","stream_url":"https://example.com/live.flv"}}"#;

        let live = parse_check_alive_live_status(body).unwrap().unwrap();

        assert_eq!(live.room_id, "7312345");
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/live.flv")
        );
    }

    #[test]
    fn proxy_url_for_reqwest_rejects_non_http_schemes() {
        let err = proxy_url_for_reqwest(Some("socks5://127.0.0.1:9000")).unwrap_err();

        assert!(err.contains("http:// or https://"));
    }
}
