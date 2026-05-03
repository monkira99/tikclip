use serde_json::Value;
use std::time::Duration;

use super::types::{LiveCheckConfig, LiveStatus, MergedLiveStatus};

const STREAM_QUALITY_ORDER: &[&str] = &["FULL_HD1", "HD1", "SD1", "SD2"];
const SDK_QUALITY_ORDER: &[&str] = &["origin", "hd", "sd", "ld"];

#[cfg_attr(not(test), allow(dead_code))]
pub fn resolve_live_status(config: &LiveCheckConfig<'_>) -> Result<Option<LiveStatus>, String> {
    let cookie_header = super::http_transport::normalize_cookie_header(config.cookies_json)?;
    let proxy_url = super::http_transport::proxy_url_for_reqwest(config.proxy_url)?;
    let timeout = Duration::from_secs(10);
    let client = super::http_transport::build_tiktok_reqwest_client(
        cookie_header.as_str(),
        proxy_url.as_deref(),
        timeout,
    )?;

    let username = config.username.trim_start_matches('@');
    if username.is_empty() {
        log::info!("tiktok live check skipped reason=empty_username");
        return Ok(None);
    }
    log::info!(
        "tiktok live check started username={} cookies_present={} proxy_present={} waf_bypass_enabled={}",
        username,
        !cookie_header.is_empty(),
        proxy_url.is_some(),
        config.waf_bypass_enabled
    );

    let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
    let live_status = runtime.block_on(async {
        if config.waf_bypass_enabled {
            log::info!("tiktok live check trying waf user room api username={}", username);
            let bypass_client = build_tiktok_waf_bypass_client(
                cookie_header.as_str(),
                proxy_url.as_deref(),
                timeout,
            )?;
            match fetch_live_status_from_user_room_api_waf(
                &bypass_client,
                username,
                cookie_header.as_str(),
            )
            .await
            {
                Ok(status) => {
                    log::info!(
                        "tiktok live check waf user room api completed username={} live={}",
                        username,
                        status.is_some()
                    );
                    return Ok::<Option<LiveStatus>, String>(status);
                }
                Err(api_err) => {
                    log::warn!(
                        "tiktok live check waf user room api failed username={} error={}",
                        username,
                        api_err
                    );
                    log::info!(
                        "tiktok live check trying signed waf fallback username={}",
                        username
                    );
                    match fetch_live_status_from_signed_user_room_api_waf(
                        &bypass_client,
                        username,
                        cookie_header.as_str(),
                    )
                    .await
                    {
                        Ok(status) => {
                            log::info!(
                                "tiktok live check signed waf fallback completed username={} live={}",
                                username,
                                status.is_some()
                            );
                            return Ok(status);
                        }
                        Err(signed_err) => {
                            log::warn!(
                                "tiktok live check signed waf fallback failed username={} error={}",
                                username,
                                signed_err
                            );
                            let combined_err = format!("{api_err}; signed fallback: {signed_err}");
                            log::info!(
                                "tiktok live check trying live HTML fallback username={}",
                                username
                            );
                            let html_status = fetch_live_status_from_live_html(
                                &client,
                                username,
                                cookie_header.as_str(),
                                Some(combined_err.as_str()),
                            )
                            .await?;
                            return Ok(html_status);
                        }
                    }
                }
            }
        }

        log::info!("tiktok live check trying user room api username={}", username);
        let api_status_result = fetch_live_status_from_user_room_api(&client, username).await;
        match api_status_result {
            Ok(status) => {
                log::info!(
                    "tiktok live check user room api completed username={} live={}",
                    username,
                    status.is_some()
                );
                Ok::<Option<LiveStatus>, String>(status)
            }
            Err(api_err) => {
                log::warn!(
                    "tiktok live check user room api failed username={} error={}",
                    username,
                    api_err
                );
                log::info!("tiktok live check trying live HTML fallback username={}", username);
                let html_status = fetch_live_status_from_live_html(
                    &client,
                    username,
                    cookie_header.as_str(),
                    Some(api_err.as_str()),
                )
                .await?;
                Ok(html_status)
            }
        }
    })?;

    log::info!(
        "tiktok live check completed username={} live={} room_id={}",
        username,
        live_status.is_some(),
        live_status
            .as_ref()
            .map(|status| status.room_id.as_str())
            .unwrap_or("")
    );
    Ok(live_status)
}

fn build_tiktok_waf_bypass_client(
    _cookie_header: &str,
    proxy_url: Option<&str>,
    timeout: Duration,
) -> Result<wreq::Client, String> {
    let mut builder = wreq::Client::builder()
        .emulation(wreq_util::Emulation::Chrome136)
        .timeout(timeout)
        .redirect(wreq::redirect::Policy::limited(10));
    if let Some(proxy) = proxy_url {
        builder = builder.proxy(wreq::Proxy::all(proxy).map_err(|e| e.to_string())?);
    }
    builder.build().map_err(|e| e.to_string())
}

fn add_waf_headers(request: wreq::RequestBuilder, cookie_header: &str) -> wreq::RequestBuilder {
    let request = request
        .header(wreq::header::ORIGIN, "https://www.tiktok.com")
        .header(wreq::header::REFERER, "https://www.tiktok.com/");
    if cookie_header.trim().is_empty() {
        request
    } else {
        request.header(wreq::header::COOKIE, cookie_header)
    }
}

async fn fetch_live_status_from_user_room_api_waf(
    client: &wreq::Client,
    username: &str,
    cookie_header: &str,
) -> Result<Option<LiveStatus>, String> {
    let response = add_waf_headers(
        client
            .get("https://www.tiktok.com/api-live/user/room/")
            .query(&[
                ("aid", "1988"),
                ("uniqueId", username),
                ("sourceType", "54"),
            ]),
        cookie_header,
    )
    .send()
    .await
    .map_err(|e| format!("waf api-live user room request failed: {e}"))?;
    let status = response.status().as_u16();
    let url = response.uri().to_string();
    let text = response
        .text()
        .await
        .map_err(|e| format!("waf api-live user room body read failed: {e}"))?;
    super::http_transport::ensure_success_status(status, &url, &text)
        .map_err(|e| format!("waf api-live user room returned {e}"))?;
    live_status_from_user_room_api_text(text.as_str())
}

async fn fetch_live_status_from_signed_user_room_api_waf(
    client: &wreq::Client,
    username: &str,
    cookie_header: &str,
) -> Result<Option<LiveStatus>, String> {
    let sign_response = add_waf_headers(
        client
            .get("https://tikrec.com/tiktok/room/api/sign")
            .query(&[("unique_id", username)]),
        cookie_header,
    )
    .send()
    .await
    .map_err(|e| format!("signed API URL request failed: {e}"))?;
    let sign_status = sign_response.status().as_u16();
    let sign_url = sign_response.uri().to_string();
    let sign_text = sign_response
        .text()
        .await
        .map_err(|e| format!("signed API URL body read failed: {e}"))?;
    super::http_transport::ensure_success_status(sign_status, &sign_url, &sign_text)
        .map_err(|e| format!("signed API URL returned {e}"))?;
    let signed_path = signed_path_from_tikrec_response(sign_text.as_str())?;
    let signed_url = if signed_path.starts_with("http://") || signed_path.starts_with("https://") {
        signed_path
    } else {
        format!("https://www.tiktok.com{signed_path}")
    };

    let response = add_waf_headers(client.get(signed_url.as_str()), cookie_header)
        .send()
        .await
        .map_err(|e| format!("signed api-live user room request failed: {e}"))?;
    let status = response.status().as_u16();
    let url = response.uri().to_string();
    let text = response
        .text()
        .await
        .map_err(|e| format!("signed api-live user room body read failed: {e}"))?;
    super::http_transport::ensure_success_status(status, &url, &text)
        .map_err(|e| format!("signed api-live user room returned {e}"))?;
    live_status_from_user_room_api_text(text.as_str())
}

fn signed_path_from_tikrec_response(text: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("signed API JSON parse failed: {e}"))?;
    value
        .get("signed_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| "signed API response missing signed_path".to_string())
}

async fn fetch_live_status_from_user_room_api(
    client: &reqwest::Client,
    username: &str,
) -> Result<Option<LiveStatus>, String> {
    let response = client
        .get("https://www.tiktok.com/api-live/user/room/")
        .query(&[
            ("aid", "1988"),
            ("uniqueId", username),
            ("sourceType", "54"),
        ])
        .send()
        .await
        .map_err(|e| format!("api-live user room request failed: {e}"))?;
    let status = response.status().as_u16();
    let url = response.url().to_string();
    let text = response
        .text()
        .await
        .map_err(|e| format!("api-live user room body read failed: {e}"))?;
    super::http_transport::ensure_success_status(status, &url, &text)
        .map_err(|e| format!("api-live user room returned {e}"))?;
    live_status_from_user_room_api_text(text.as_str())
}

async fn fetch_live_status_from_live_html(
    client: &reqwest::Client,
    username: &str,
    cookie_header: &str,
    api_error: Option<&str>,
) -> Result<Option<LiveStatus>, String> {
    let response = match client
        .get(format!("https://www.tiktok.com/@{username}/live"))
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => return Err(format!("live HTML request failed: {err}")),
    };
    let status = response.status().as_u16();
    let url = response.url().to_string();
    let text = response
        .text()
        .await
        .map_err(|e| format!("live HTML body read failed: {e}"))?;
    super::http_transport::ensure_success_status(status, &url, &text)
        .map_err(|e| format!("live HTML returned {e}"))?;
    let Some(room_id) = extract_room_id_from_live_html(text.as_str()) else {
        return Err(api_error.map_or_else(
            || "could not resolve TikTok room id from live HTML".to_string(),
            |api_error| {
                format!(
                    "could not resolve TikTok room id from api-live or live HTML; api-live error: {api_error}"
                )
            },
        ));
    };

    let room_info_response = match client
        .get("https://webcast.tiktok.com/webcast/room/info/")
        .query(&[("aid", "1988"), ("room_id", room_id.as_str())])
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => return Err(format!("room/info request failed: {err}")),
    };
    let status = room_info_response.status().as_u16();
    let text = room_info_response.text().await.unwrap_or_default();
    let room_payload = if (200..300).contains(&status) {
        room_payload_from_room_info_text(text.as_str(), room_id.as_str())
    } else {
        Value::Null
    };

    let region = webcast_region_hint(Some(cookie_header));
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
            let parsed: Value = serde_json::from_str(alive_text.as_str()).unwrap_or(Value::Null);
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
}

fn live_status_from_user_room_api_text(text: &str) -> Result<Option<LiveStatus>, String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|e| format!("api-live user room JSON parse failed: {e}"))?;
    if value
        .get("message")
        .and_then(Value::as_str)
        .is_some_and(|message| message == "user_not_found")
    {
        return Ok(None);
    }

    let data = value
        .get("data")
        .ok_or_else(|| "api-live user room response missing data".to_string())?;
    let user = data.get("user").unwrap_or(&Value::Null);
    let live_room = data.get("liveRoom").unwrap_or(&Value::Null);
    let status = live_room
        .get("status")
        .and_then(value_as_i64)
        .or_else(|| user.get("status").and_then(value_as_i64));

    let Some(status) = status else {
        return Err("api-live user room response did not contain a live status".to_string());
    };
    if status != 2 {
        return Ok(None);
    }

    let room_id = user
        .get("roomId")
        .and_then(Value::as_str)
        .or_else(|| user.get("room_id").and_then(Value::as_str))
        .or_else(|| live_room.get("roomId").and_then(Value::as_str))
        .or_else(|| live_room.get("room_id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            live_room
                .get("id")
                .and_then(value_as_i64)
                .map(|value| value.to_string())
        })
        .ok_or_else(|| "api-live user room response missing roomId".to_string())?;

    Ok(Some(LiveStatus {
        room_id,
        stream_url: pick_stream_url_from_user_room_payload(data),
        viewer_count: live_room
            .get("liveRoomStats")
            .and_then(|value| value.get("userCount"))
            .and_then(value_as_i64)
            .or_else(|| viewer_count_from_sdk_pull_data(live_room.get("streamData"))),
    }))
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
    if let Some(url) = stream_url
        .get("live_core_sdk_data")
        .and_then(|value| value.get("pull_data"))
        .and_then(pick_stream_url_from_sdk_pull_data)
    {
        return Some(url);
    }
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

fn pick_stream_url_from_user_room_payload(data: &Value) -> Option<String> {
    let live_room = data.get("liveRoom")?;
    live_room
        .get("streamData")
        .and_then(pick_stream_url_from_sdk_pull_data)
        .or_else(|| {
            live_room
                .get("hevcStreamData")
                .and_then(pick_stream_url_from_sdk_pull_data)
        })
        .or_else(|| pick_stream_url_from_room_payload(live_room))
}

fn pick_stream_url_from_sdk_pull_data(pull_data_parent: &Value) -> Option<String> {
    let pull_data = pull_data_parent
        .get("pull_data")
        .unwrap_or(pull_data_parent);
    let stream_data = pull_data.get("stream_data").and_then(Value::as_str)?;
    let parsed: Value = serde_json::from_str(stream_data).ok()?;
    let streams = parsed.get("data")?.as_object()?;

    let quality_levels = pull_data
        .get("options")
        .and_then(|value| value.get("qualities"))
        .and_then(Value::as_array)
        .map(|qualities| {
            qualities
                .iter()
                .filter_map(|quality| {
                    let key = quality.get("sdk_key")?.as_str()?.to_string();
                    let level = quality.get("level").and_then(value_as_i64).unwrap_or(-1);
                    Some((key, level))
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut best: Option<(i64, String)> = None;
    for (sdk_key, entry) in streams {
        let level = quality_levels
            .get(sdk_key)
            .copied()
            .or_else(|| {
                SDK_QUALITY_ORDER
                    .iter()
                    .rev()
                    .position(|candidate| *candidate == sdk_key)
                    .and_then(|index| i64::try_from(index).ok())
            })
            .unwrap_or(-1);
        let Some(url) = entry
            .get("main")
            .and_then(|main| main.get("flv").or_else(|| main.get("hls")))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
        else {
            continue;
        };
        if best
            .as_ref()
            .is_none_or(|(best_level, _)| level > *best_level)
        {
            best = Some((level, url.to_string()));
        }
    }

    best.map(|(_, url)| url)
}

fn viewer_count_from_sdk_pull_data(stream_data: Option<&Value>) -> Option<i64> {
    let stream_data = stream_data?;
    let pull_data = stream_data.get("pull_data").unwrap_or(stream_data);
    let raw = pull_data.get("stream_data").and_then(Value::as_str)?;
    let parsed: Value = serde_json::from_str(raw).ok()?;
    parsed
        .get("common")
        .and_then(|value| value.get("user_count"))
        .and_then(value_as_i64)
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
        extract_room_id_from_live_html, live_status_from_user_room_api_text,
        merge_live_status_from_room_payload, pick_stream_url_from_room_payload,
        signed_path_from_tikrec_response, webcast_region_hint,
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
    fn live_status_from_user_room_api_text_reads_room_status_and_sdk_stream_data() {
        let stream_data = serde_json::json!({
            "common": {
                "user_count": 98
            },
            "data": {
                "origin": {
                    "main": {
                        "flv": "https://example.com/origin.flv"
                    }
                },
                "hd": {
                    "main": {
                        "flv": "https://example.com/hd.flv"
                    }
                }
            }
        })
        .to_string();
        let body = json!({
            "data": {
                "user": {
                    "roomId": "7632683733313325845",
                    "status": 2
                },
                "liveRoom": {
                    "status": 2,
                    "liveRoomStats": {
                        "userCount": 174
                    },
                    "streamData": {
                        "pull_data": {
                            "options": {
                                "qualities": [
                                    { "sdk_key": "origin", "level": 10 },
                                    { "sdk_key": "hd", "level": 3 }
                                ]
                            },
                            "stream_data": stream_data
                        }
                    }
                }
            },
            "message": "",
            "statusCode": 0
        });

        let live = live_status_from_user_room_api_text(&body.to_string())
            .unwrap()
            .unwrap();

        assert_eq!(live.room_id, "7632683733313325845");
        assert_eq!(
            live.stream_url.as_deref(),
            Some("https://example.com/origin.flv")
        );
        assert_eq!(live.viewer_count, Some(174));
    }

    #[test]
    fn live_status_from_user_room_api_text_returns_none_for_explicit_offline_status() {
        let body = json!({
            "data": {
                "user": {
                    "roomId": "7632683733313325845",
                    "status": 4
                },
                "liveRoom": {
                    "status": 4
                }
            },
            "message": "",
            "statusCode": 0
        });

        let live = live_status_from_user_room_api_text(&body.to_string()).unwrap();

        assert_eq!(live, None);
    }

    #[test]
    fn signed_path_from_tikrec_response_reads_signed_path() {
        let signed_path = signed_path_from_tikrec_response(
            r#"{"signed_path":"/api-live/user/room/?uniqueId=shop_abc&X-Bogus=abc"}"#,
        )
        .unwrap();

        assert_eq!(
            signed_path,
            "/api-live/user/room/?uniqueId=shop_abc&X-Bogus=abc"
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
