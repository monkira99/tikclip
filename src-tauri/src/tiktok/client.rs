use serde_json::Value;

use super::check_live::{
    extract_room_id_from_live_html, merge_live_status_from_room_payload,
    pick_stream_url_from_room_payload,
};
use super::types::LiveStatus;

#[cfg_attr(not(test), allow(dead_code))]
pub fn normalize_cookie_header(raw: &str) -> Result<String, String> {
    super::http_transport::normalize_cookie_header(raw)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn proxy_url_for_reqwest(raw: Option<&str>) -> Result<Option<String>, String> {
    super::http_transport::proxy_url_for_reqwest(raw)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn extract_room_id_from_html(html: &str) -> Option<String> {
    extract_room_id_from_live_html(html)
}

#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_room_info_live_status(body: &str) -> Result<Option<LiveStatus>, String> {
    let value: Value = serde_json::from_str(body).map_err(|e| e.to_string())?;
    let room = &value["data"]["room"];
    let merged = merge_live_status_from_room_payload(room);
    if !merged.is_live {
        return Ok(None);
    }

    let room_id = merged.room_id.unwrap_or_default();
    let stream_url =
        pick_stream_url_from_room_payload(room).ok_or_else(|| "missing stream_url".to_string())?;

    Ok(Some(LiveStatus {
        room_id,
        stream_url: Some(stream_url),
        viewer_count: merged.viewer_count,
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

    let merged = merge_live_status_from_room_payload(first_row);
    if !merged.is_live {
        return Ok(None);
    }

    Ok(Some(LiveStatus {
        room_id: merged.room_id.unwrap_or_default(),
        stream_url: merged.stream_url,
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
