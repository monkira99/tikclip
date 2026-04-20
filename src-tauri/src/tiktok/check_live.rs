use serde_json::Value;

use super::types::{LiveCheckConfig, LiveStatus, MergedLiveStatus};

const STREAM_QUALITY_ORDER: &[&str] = &["FULL_HD1", "HD1", "SD1", "SD2"];

#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_live_status(config: &LiveCheckConfig<'_>) -> Result<Option<LiveStatus>, String> {
    let cookie_header = super::http_transport::normalize_cookie_header(config.cookies_json)?;
    let proxy_url = super::http_transport::proxy_url_for_reqwest(config.proxy_url)?;
    let client = super::http_transport::build_tiktok_reqwest_client(
        cookie_header.as_str(),
        proxy_url.as_deref(),
        std::time::Duration::from_secs(10),
    )?;

    let username = config.username.trim_start_matches('@');
    if username.is_empty() {
        return Ok(None);
    }

    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let live_status = runtime.block_on(async {
        let response = match client
            .get(format!("https://www.tiktok.com/@{username}/live"))
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => return Ok::<Option<LiveStatus>, String>(None),
        };
        let status = response.status().as_u16();
        let url = response.url().to_string();
        let text = match response.text().await {
            Ok(text) => text,
            Err(_) => return Ok(None),
        };
        if super::http_transport::ensure_success_status(status, &url, &text).is_err() {
            return Ok(None);
        }
        let Some(room_id) = extract_room_id_from_live_html(text.as_str()) else {
            return Ok::<Option<LiveStatus>, String>(None);
        };

        let room_info_response = match client
            .get("https://webcast.tiktok.com/webcast/room/info/")
            .query(&[("aid", "1988"), ("room_id", room_id.as_str())])
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => return Ok(None),
        };
        let status = room_info_response.status().as_u16();
        let text = room_info_response.text().await.unwrap_or_default();
        let room_payload = if (200..300).contains(&status) {
            room_payload_from_room_info_text(text.as_str(), room_id.as_str())
        } else {
            Value::Null
        };

        let region = webcast_region_hint(Some(cookie_header.as_str()));
        let check_alive_live = match client
            .get("https://webcast.tiktok.com/webcast/room/check_alive/")
            .query(&[
                ("aid", "1988"),
                ("region", region),
                ("room_ids", room_id.as_str()),
                ("user_is_login", "true"),
            ])
            .send()
            .await
        {
            Ok(check_alive_response) if check_alive_response.status().is_success() => {
                let alive_text = check_alive_response.text().await.unwrap_or_default();
                let parsed: Value =
                    serde_json::from_str(alive_text.as_str()).unwrap_or(Value::Null);
                parsed
                    .get("data")
                    .and_then(Value::as_array)
                    .and_then(|rows| rows.first())
                    .and_then(|row| row.get("alive"))
                    .and_then(value_as_i64)
                    .is_some_and(|flag| flag == 1)
            }
            _ => false,
        };

        let room_payload = if room_payload.is_null() {
            fallback_room_payload(room_id.as_str(), check_alive_live)
        } else {
            room_payload
        };

        let merged = merge_live_status_from_room_payload(&room_payload);
        if !(merged.is_live || check_alive_live) {
            return Ok(None);
        }

        Ok(Some(LiveStatus {
            room_id: merged.room_id.unwrap_or(room_id),
            stream_url: merged.stream_url,
            viewer_count: merged.viewer_count,
        }))
    })?;

    Ok(live_status)
}

fn fallback_room_payload(room_id: &str, is_live: bool) -> Value {
    serde_json::json!({
        "LiveRoomInfo": { "status": if is_live { 2 } else { 4 } },
        "room_id": room_id,
    })
}

fn room_payload_from_room_info_text(text: &str, room_id: &str) -> Value {
    let value: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let room = value
        .get("data")
        .and_then(Value::as_object)
        .and_then(|data| data.get("room").and_then(Value::as_object).or(Some(data)))
        .cloned()
        .unwrap_or_default();
    let mut merged = serde_json::Map::new();
    merged.extend(room);
    merged.insert("room_id".to_string(), Value::String(room_id.to_string()));
    Value::Object(merged)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn extract_room_id_from_live_html(html: &str) -> Option<String> {
    const PREFIXES: &[&str] = &[
        "\"roomId\":\"",
        "\"room_id\":\"",
        "\\\"roomId\\\":\\\"",
        "\\\"room_id\\\":\\\"",
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

    for key in ["\"roomId\"", "\"room_id\""] {
        if let Some(room_id) = extract_digits_after_json_key(html, key) {
            return Some(room_id);
        }
    }

    None
}

fn extract_digits_after_json_key(html: &str, key: &str) -> Option<String> {
    let mut from = 0usize;
    while let Some(relative) = html[from..].find(key) {
        let start = from + relative + key.len();
        let mut rest = &html[start..];

        rest = rest.trim_start_matches(|ch: char| ch.is_ascii_whitespace());
        if !rest.starts_with(':') {
            from = start;
            continue;
        }
        rest = &rest[1..];
        rest = rest.trim_start_matches(|ch: char| ch.is_ascii_whitespace());

        if let Some(stripped) = rest.strip_prefix('"') {
            let digits: String = stripped
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect();
            if digits.len() >= 5 {
                return Some(digits);
            }
        } else {
            let digits: String = rest.chars().take_while(|ch| ch.is_ascii_digit()).collect();
            if digits.len() >= 5 {
                return Some(digits);
            }
        }

        from = start;
    }

    None
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn webcast_region_hint(cookie_header: Option<&str>) -> &'static str {
    let Some(cookies) = cookie_header else {
        return "CH";
    };

    let idc = cookies
        .split(';')
        .find_map(|part| {
            let mut pieces = part.trim().splitn(2, '=');
            let key = pieces.next()?.trim().to_ascii_lowercase();
            let value = pieces.next()?.trim();
            if key == "tt-target-idc" {
                Some(value.to_ascii_lowercase())
            } else {
                None
            }
        })
        .unwrap_or_default();

    if idc.contains("alisg") {
        return "SG";
    }
    if idc.contains("useast") {
        return "US";
    }
    if idc.contains("eu") || idc.contains("gcp") {
        return "EU";
    }

    "CH"
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn pick_stream_url_from_room_payload(room: &Value) -> Option<String> {
    let stream_url = room.get("stream_url")?;
    if let Some(raw) = stream_url.as_str().filter(|value| !value.is_empty()) {
        return Some(raw.to_string());
    }
    if !stream_url.is_object() {
        return None;
    }

    let flv_pull = stream_url.get("flv_pull_url");
    if let Some(url) = pick_url_from_quality_map(flv_pull) {
        return Some(url);
    }

    let hls_pull = stream_url
        .get("hls_pull_url_map")
        .or_else(|| stream_url.get("hls_pull_url"));
    if let Some(url) = pick_url_from_quality_map(hls_pull) {
        return Some(url);
    }

    let raw_flv = stream_url.get("flv_pull_url").and_then(Value::as_str);
    if let Some(url) = raw_flv.filter(|value| !value.is_empty()) {
        return Some(url.to_string());
    }

    let raw_hls = stream_url.get("hls_pull_url").and_then(Value::as_str);
    raw_hls
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn merge_live_status_from_room_payload(room: &Value) -> MergedLiveStatus {
    let is_live = status_int_from_room(room).is_some_and(|status| status == 2)
        || room
            .get("alive")
            .and_then(value_as_i64)
            .is_some_and(|alive| alive == 1);
    let room_id = room_id_from_room_payload(room);

    MergedLiveStatus {
        is_live,
        room_id,
        title: if is_live {
            title_from_room_payload(room)
        } else {
            None
        },
        stream_url: if is_live {
            pick_stream_url_from_room_payload(room)
        } else {
            None
        },
        viewer_count: if is_live {
            viewer_count_from_room_payload(room)
        } else {
            None
        },
    }
}

fn pick_url_from_quality_map(value: Option<&Value>) -> Option<String> {
    let map = value?.as_object()?;
    for quality in STREAM_QUALITY_ORDER {
        if let Some(url) = map.get(*quality).and_then(Value::as_str) {
            if !url.is_empty() {
                return Some(url.to_string());
            }
        }
    }
    map.values()
        .filter_map(Value::as_str)
        .find(|url| !url.is_empty())
        .map(ToString::to_string)
}

fn value_as_i64(value: &Value) -> Option<i64> {
    if let Some(number) = value.as_i64() {
        return Some(number);
    }
    value.as_str().and_then(|text| text.parse::<i64>().ok())
}

fn status_int_from_room(room: &Value) -> Option<i64> {
    room.get("LiveRoomInfo")
        .and_then(|value| value.get("status"))
        .and_then(value_as_i64)
        .or_else(|| room.get("status").and_then(value_as_i64))
}

fn room_id_from_room_payload(room: &Value) -> Option<String> {
    for field in ["id_str", "room_id", "web_rid"] {
        if let Some(value) = room
            .get(field)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_string());
        }
        if let Some(value) = room.get(field).and_then(value_as_i64) {
            return Some(value.to_string());
        }
    }
    None
}

fn viewer_count_from_room_payload(room: &Value) -> Option<i64> {
    room.get("LiveRoomInfo")
        .and_then(|value| value.get("liveRoomStats"))
        .and_then(|value| value.get("userCount"))
        .and_then(value_as_i64)
        .or_else(|| room.get("owner_count").and_then(value_as_i64))
        .or_else(|| room.get("user_count").and_then(value_as_i64))
        .or_else(|| room.get("viewer_count").and_then(value_as_i64))
}

fn title_from_room_payload(room: &Value) -> Option<String> {
    for title in [
        room.get("LiveRoomInfo")
            .and_then(|value| value.get("title"))
            .and_then(Value::as_str),
        room.get("LiveRoomInfo")
            .and_then(|value| value.get("liveRoomName"))
            .and_then(Value::as_str),
        room.get("LiveRoomInfo")
            .and_then(|value| value.get("liveRoomTitle"))
            .and_then(Value::as_str),
        room.get("title").and_then(Value::as_str),
    ] {
        if let Some(clean) = title.map(str::trim).filter(|value| !value.is_empty()) {
            return Some(clean.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        extract_room_id_from_live_html, merge_live_status_from_room_payload,
        pick_stream_url_from_room_payload, webcast_region_hint,
    };

    #[test]
    fn extract_room_id_from_live_html_reads_known_patterns() {
        assert_eq!(
            extract_room_id_from_live_html(r#"...\"room_id\":\"7312345\"..."#),
            Some("7312345".to_string())
        );
        assert_eq!(
            extract_room_id_from_live_html(r#"...room/731234567890..."#),
            Some("731234567890".to_string())
        );
    }

    #[test]
    fn extract_room_id_from_live_html_supports_whitespace_json_variants() {
        assert_eq!(
            extract_room_id_from_live_html(r#"..."roomId"   :   "7412345"..."#),
            Some("7412345".to_string())
        );
        assert_eq!(
            extract_room_id_from_live_html("...\"room_id\"\n:\t\"7512345\"..."),
            Some("7512345".to_string())
        );
        assert_eq!(
            extract_room_id_from_live_html(r#"..."room_id"   :   7612345..."#),
            Some("7612345".to_string())
        );
    }

    #[test]
    fn webcast_region_hint_maps_tiktok_target_idc() {
        assert_eq!(
            webcast_region_hint(Some("sessionid=abc; tt-target-idc=useast2a")),
            "US"
        );
        assert_eq!(
            webcast_region_hint(Some("sessionid=abc; tt-target-idc=alisg1")),
            "SG"
        );
        assert_eq!(
            webcast_region_hint(Some("sessionid=abc; tt-target-idc=eu-central")),
            "EU"
        );
        assert_eq!(webcast_region_hint(Some("sessionid=abc")), "CH");
    }

    #[test]
    fn pick_stream_url_from_room_payload_prefers_flv_quality_order() {
        let room = json!({
            "stream_url": {
                "flv_pull_url": {
                    "SD1": "https://example.com/sd.flv",
                    "FULL_HD1": "https://example.com/fhd.flv"
                },
                "hls_pull_url_map": {
                    "HD1": "https://example.com/hd.m3u8"
                }
            }
        });

        assert_eq!(
            pick_stream_url_from_room_payload(&room).as_deref(),
            Some("https://example.com/fhd.flv")
        );
    }

    #[test]
    fn pick_stream_url_from_room_payload_falls_back_to_hls_and_raw_strings() {
        let with_hls = json!({
            "stream_url": {
                "hls_pull_url_map": {
                    "HD1": "https://example.com/hd.m3u8"
                }
            }
        });
        assert_eq!(
            pick_stream_url_from_room_payload(&with_hls).as_deref(),
            Some("https://example.com/hd.m3u8")
        );

        let with_raw = json!({
            "stream_url": {
                "hls_pull_url": "https://example.com/raw.m3u8"
            }
        });
        assert_eq!(
            pick_stream_url_from_room_payload(&with_raw).as_deref(),
            Some("https://example.com/raw.m3u8")
        );
    }

    #[test]
    fn pick_stream_url_from_room_payload_falls_back_to_first_unknown_quality_value() {
        let room = json!({
            "stream_url": {
                "flv_pull_url": {
                    "CUSTOM_QUALITY": "https://example.com/custom.flv"
                }
            }
        });

        assert_eq!(
            pick_stream_url_from_room_payload(&room).as_deref(),
            Some("https://example.com/custom.flv")
        );
    }

    #[test]
    fn pick_stream_url_from_room_payload_skips_empty_unknown_quality_values() {
        let room = json!({
            "stream_url": {
                "flv_pull_url": {
                    "FIRST_UNKNOWN": "",
                    "SECOND_UNKNOWN": "https://example.com/fallback.flv"
                }
            }
        });

        assert_eq!(
            pick_stream_url_from_room_payload(&room).as_deref(),
            Some("https://example.com/fallback.flv")
        );
    }

    #[test]
    fn merge_live_status_from_room_payload_reads_room_info_shape() {
        let room = json!({
            "status": 2,
            "id_str": "7312345",
            "title": "Live now",
            "owner_count": 321,
            "stream_url": {
                "flv_pull_url": {
                    "FULL_HD1": "https://example.com/live.flv"
                }
            }
        });

        let live = merge_live_status_from_room_payload(&room);

        assert!(live.is_live);
        assert_eq!(live.room_id.as_deref(), Some("7312345"));
        assert_eq!(live.title.as_deref(), Some("Live now"));
        assert_eq!(live.viewer_count, Some(321));
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/live.flv")
        );
    }

    #[test]
    fn merge_live_status_from_room_payload_uses_check_alive_hint_when_status_missing() {
        let room = json!({
            "alive": 1,
            "room_id": "998877",
            "stream_url": "https://example.com/live.flv"
        });

        let live = merge_live_status_from_room_payload(&room);

        assert!(live.is_live);
        assert_eq!(live.room_id.as_deref(), Some("998877"));
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/live.flv")
        );
    }

    #[test]
    fn merge_live_status_from_room_payload_marks_not_live_when_hints_are_offline() {
        let room = json!({
            "LiveRoomInfo": {
                "status": 4
            },
            "room_id": "112233"
        });

        let live = merge_live_status_from_room_payload(&room);

        assert!(!live.is_live);
        assert_eq!(live.room_id.as_deref(), Some("112233"));
        assert_eq!(live.stream_url, None);
        assert_eq!(live.viewer_count, None);
        assert_eq!(live.title, None);
    }

    #[test]
    fn merge_live_status_from_room_payload_preserves_leading_zero_room_id_string() {
        let room = json!({
            "status": 2,
            "room_id": "0012345",
            "stream_url": {
                "flv_pull_url": {
                    "FULL_HD1": "https://example.com/live.flv"
                }
            }
        });

        let live = merge_live_status_from_room_payload(&room);

        assert_eq!(live.room_id.as_deref(), Some("0012345"));
    }
}
