use super::http_transport::build_tiktok_reqwest_client;
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

const MAX_PRODUCT_VIDEOS: usize = 8;
const MAX_IMAGE_BYTES: usize = 25 * 1024 * 1024;
const MAX_VIDEO_BYTES: usize = 80 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FetchedProductMediaFile {
    pub kind: String,
    pub path: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct FetchedProductData {
    pub name: Option<String>,
    pub description: Option<String>,
    pub price: Option<f64>,
    pub image_url: Option<String>,
    pub category: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub image_urls: Vec<String>,
    pub video_urls: Vec<String>,
    pub media_files: Vec<FetchedProductMediaFile>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FetchProductResponse {
    pub success: bool,
    pub incomplete: bool,
    pub data: Option<FetchedProductData>,
    pub error: Option<String>,
}

pub async fn fetch_product_from_url(
    storage_root: &Path,
    url: &str,
    cookies_json: Option<&str>,
    download_media: bool,
) -> FetchProductResponse {
    let url_s = url.trim();
    if url_s.is_empty() {
        log::info!("product fetch rejected: empty URL");
        return FetchProductResponse {
            success: false,
            incomplete: false,
            data: None,
            error: Some("URL is required".to_string()),
        };
    }

    let cookie_header = cookie_header_from_json(cookies_json.unwrap_or(""));
    log::info!(
        "product fetch started url={} cookies_present={} download_media={}",
        safe_url_label(url_s),
        !cookie_header.is_empty(),
        download_media
    );
    let client =
        match build_tiktok_reqwest_client(cookie_header.as_str(), None, Duration::from_secs(20)) {
            Ok(client) => client,
            Err(err) => {
                log::warn!("product fetch client build failed: {}", err);
                return FetchProductResponse {
                    success: false,
                    incomplete: true,
                    data: Some(FetchedProductData::default()),
                    error: Some(err),
                };
            }
        };

    let response = match client.get(url_s).send().await {
        Ok(response) => response,
        Err(_) => {
            log::warn!(
                "product fetch network request failed url={}",
                safe_url_label(url_s)
            );
            return incomplete_response("Network error while loading the page");
        }
    };
    let status = response.status();
    let final_url = response.url().to_string();
    log::info!(
        "product fetch response received status={} final_url={}",
        status.as_u16(),
        safe_url_label(final_url.as_str())
    );
    if !status.is_success() {
        log::warn!(
            "product fetch incomplete: HTTP {} from {}",
            status.as_u16(),
            safe_url_label(final_url.as_str())
        );
        return incomplete_response(format!("HTTP {} from TikTok", status.as_u16()).as_str());
    }
    let html = match response.text().await {
        Ok(text) => text,
        Err(_) => {
            log::warn!("product fetch failed reading response body");
            return incomplete_response("Network error while loading the page");
        }
    };
    log::info!("product fetch HTML loaded bytes={}", html.len());

    let mut data = parse_product_html(html.as_str(), final_url.as_str());
    log::info!(
        "product fetch parsed fields name_present={} images={} videos={} tiktok_shop_id_present={}",
        data.name.is_some(),
        data.image_urls.len(),
        data.video_urls.len(),
        data.tiktok_shop_id.is_some()
    );
    if html_suggests_security_challenge(html.as_str()) {
        log::warn!("product fetch blocked by TikTok security challenge");
        return FetchProductResponse {
            success: false,
            incomplete: true,
            data: Some(data),
            error: Some(
                "TikTok returned a security or captcha page. Paste cookies from a logged-in browser (optional field) or try again later."
                    .to_string(),
            ),
        };
    }

    if download_media && (!data.image_urls.is_empty() || !data.video_urls.is_empty()) {
        let batch = Uuid::new_v4()
            .simple()
            .to_string()
            .chars()
            .take(16)
            .collect::<String>();
        let out_dir = storage_root.join("products").join("fetched").join(batch);
        if std::fs::create_dir_all(&out_dir).is_ok() {
            log::info!(
                "product fetch downloading media images={} videos={} out_dir={}",
                data.image_urls.len(),
                data.video_urls.len(),
                out_dir.display()
            );
            let mut items = data
                .image_urls
                .iter()
                .map(|url| ("image", url.as_str()))
                .collect::<Vec<_>>();
            items.extend(data.video_urls.iter().map(|url| ("video", url.as_str())));
            data.media_files = download_media_items(&client, &out_dir, &items).await;
            log::info!(
                "product fetch media download completed files={}",
                data.media_files.len()
            );
            if let Some(first_image) = data
                .media_files
                .iter()
                .find(|media| media.kind == "image")
                .map(|media| media.path.clone())
            {
                data.image_url = Some(first_image);
            }
        } else {
            log::warn!(
                "product fetch media output directory could not be created: {}",
                out_dir.display()
            );
        }
    }

    let success = data.name.is_some();
    let error = if success {
        None
    } else {
        Some("Could not find product title on this page (layout may have changed).".to_string())
    };
    log::info!(
        "product fetch completed success={} incomplete={} media_files={}",
        success,
        !success,
        data.media_files.len()
    );
    FetchProductResponse {
        success,
        incomplete: !success,
        data: Some(data),
        error,
    }
}

fn safe_url_label(raw: &str) -> String {
    match reqwest::Url::parse(raw) {
        Ok(url) => {
            let host = url.host_str().unwrap_or("unknown-host");
            format!("{}://{}{}", url.scheme(), host, url.path())
        }
        Err(_) => "<invalid-url>".to_string(),
    }
}

fn incomplete_response(message: &str) -> FetchProductResponse {
    FetchProductResponse {
        success: false,
        incomplete: true,
        data: Some(FetchedProductData::default()),
        error: Some(message.to_string()),
    }
}

fn cookie_header_from_json(raw: &str) -> String {
    super::http_transport::normalize_cookie_header(raw).unwrap_or_default()
}

fn parse_product_html(html: &str, final_url: &str) -> FetchedProductData {
    let og = parse_og_tags(html);
    let mut out = FetchedProductData {
        name: og.get("title").cloned(),
        description: og.get("description").cloned(),
        image_url: og.get("image").cloned(),
        tiktok_shop_id: product_id_from_final_url(final_url),
        ..Default::default()
    };
    if out.name.is_none() {
        out.name = title_tag_product_name(html);
    }
    apply_json_ld_product(html, &mut out);
    if let Some(product_model) = parse_product_model_from_html(html) {
        if let Some(description) = description_from_product_model(&product_model) {
            out.description = Some(description);
        }
        out.image_urls = gallery_image_urls_from_product_model(&product_model);
        out.video_urls =
            video_urls_from_product_model_videos(product_model.get("videos"), MAX_PRODUCT_VIDEOS);
    } else if out
        .image_url
        .as_deref()
        .is_some_and(|url| url.starts_with("http"))
    {
        out.image_urls = vec![out.image_url.clone().unwrap_or_default()];
    }
    out
}

fn html_suggests_security_challenge(html: &str) -> bool {
    let h = html.to_ascii_lowercase();
    h.contains("security check")
        || h.contains("captcha_container")
        || h.contains("wafchallenge")
        || h.contains("please wait")
}

fn parse_og_tags(html: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for tag in find_tags(html, "meta") {
        let attrs = parse_attrs(tag);
        let Some(property) = attrs.get("property") else {
            continue;
        };
        let Some(key) = property.strip_prefix("og:") else {
            continue;
        };
        if let Some(content) = attrs.get("content") {
            out.entry(key.to_ascii_lowercase())
                .or_insert_with(|| html_unescape_basic(content));
        }
    }
    out
}

fn title_tag_product_name(html: &str) -> Option<String> {
    let raw = find_element_text(html, "title")?;
    let mut title = collapse_spaces(raw.as_str());
    for suffix in [
        " - TikTok Shop Vietnam",
        " - TikTok Shop",
        " | TikTok Shop",
        " - TikTok",
    ] {
        if title.ends_with(suffix) {
            title.truncate(title.len() - suffix.len());
            title = title.trim().to_string();
        }
    }
    (!title.is_empty()).then_some(title)
}

fn product_id_from_final_url(final_url: &str) -> Option<String> {
    let marker = "/view/product/";
    let (_, rest) = final_url.split_once(marker)?;
    let id = rest
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    (!id.is_empty()).then_some(id)
}

fn apply_json_ld_product(html: &str, out: &mut FetchedProductData) {
    for script in find_script_text_by_type(html, "application/ld+json") {
        let Ok(value) = serde_json::from_str::<Value>(script.trim()) else {
            continue;
        };
        let product = if value.get("@type").and_then(Value::as_str) == Some("Product") {
            Some(&value)
        } else {
            None
        };
        let Some(product) = product else {
            continue;
        };
        if out.name.is_none() {
            out.name = product
                .get("name")
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }
        if out.description.is_none() {
            out.description = product
                .get("description")
                .and_then(Value::as_str)
                .map(ToString::to_string);
        }
        if out.image_url.is_none() {
            out.image_url = product.get("image").and_then(|image| {
                image
                    .as_str()
                    .map(ToString::to_string)
                    .or_else(|| image.as_array()?.first()?.as_str().map(ToString::to_string))
            });
        }
        out.category = out.category.take().or_else(|| {
            product
                .get("category")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        });
        if out.price.is_none() {
            out.price = product
                .get("offers")
                .and_then(|offers| offers.get("price"))
                .and_then(|price| price.as_f64().or_else(|| price.as_str()?.parse().ok()));
        }
        if out.tiktok_shop_id.is_none() {
            out.tiktok_shop_id = product.get("sku").map(|sku| {
                sku.as_str()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| sku.to_string())
            });
        }
        break;
    }
}

fn parse_product_model_from_html(html: &str) -> Option<Value> {
    let mut best = None;
    let mut best_len = 0;
    for script in find_script_text_by_type(html, "application/json") {
        if !script.contains("loaderData") {
            continue;
        }
        let Ok(root) = serde_json::from_str::<Value>(script.trim()) else {
            continue;
        };
        let Some(product_model) = find_product_model_in_loader(&root) else {
            continue;
        };
        let image_count = product_model
            .get("images")
            .and_then(Value::as_array)
            .map(Vec::len)
            .unwrap_or(0);
        if image_count >= best_len {
            best_len = image_count;
            best = Some(product_model.clone());
        }
    }
    best
}

fn find_product_model_in_loader(root: &Value) -> Option<&Value> {
    let loader = root.get("loaderData")?.as_object()?;
    for page in loader.values() {
        let components = page.get("page_config")?.get("components_map")?.as_array()?;
        for component in components {
            let product_model = component
                .get("component_data")
                .and_then(|value| value.get("product_info"))
                .and_then(|value| value.get("product_model"))?;
            if product_model.get("product_id").is_some() {
                return Some(product_model);
            }
        }
    }
    None
}

fn gallery_image_urls_from_product_model(product_model: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let Some(images) = product_model.get("images").and_then(Value::as_array) else {
        return out;
    };
    for item in images {
        let Some(urls) = item.get("url_list").and_then(Value::as_array) else {
            continue;
        };
        for url in urls {
            let Some(url) = url.as_str().filter(|url| url.starts_with("http")) else {
                continue;
            };
            if seen.insert(url.to_string()) {
                out.push(url.to_string());
                break;
            }
        }
    }
    out
}

fn video_urls_from_product_model_videos(videos: Option<&Value>, limit: usize) -> Vec<String> {
    fn walk(value: &Value, limit: usize, out: &mut Vec<String>, seen: &mut BTreeSet<String>) {
        if out.len() >= limit {
            return;
        }
        match value {
            Value::Object(object) => {
                for child in object.values() {
                    walk(child, limit, out, seen);
                }
            }
            Value::Array(rows) => {
                for child in rows {
                    walk(child, limit, out, seen);
                }
            }
            Value::String(s) => {
                let lower = s.to_ascii_lowercase();
                if s.starts_with("http")
                    && (lower.contains(".mp4") || lower.contains(".m3u8"))
                    && seen.insert(s.to_string())
                {
                    out.push(s.to_string());
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(videos) = videos {
        walk(videos, limit, &mut out, &mut seen);
    }
    out
}

fn description_from_product_model(product_model: &Value) -> Option<String> {
    let raw = product_model.get("description")?;
    if let Some(rows) = raw.as_array() {
        return flatten_description_blocks(rows);
    }
    let raw_str = raw.as_str()?.trim();
    if raw_str.is_empty() {
        return None;
    }
    if !raw_str.starts_with('[') {
        return Some(raw_str.to_string());
    }
    let parsed = serde_json::from_str::<Value>(raw_str).ok()?;
    flatten_description_blocks(parsed.as_array()?)
}

fn flatten_description_blocks(blocks: &[Value]) -> Option<String> {
    let mut chunks = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    chunks.push(text.to_string());
                }
            }
            Some("image") => chunks.push("\n".to_string()),
            _ => {}
        }
    }
    let mut out = String::new();
    for chunk in chunks {
        if chunk == "\n" {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            continue;
        }
        let starts_list =
            chunk.trim_start().starts_with("\\-") || chunk.trim_start().starts_with('*');
        if !out.is_empty() && starts_list && !out.ends_with('\n') {
            out.push('\n');
        } else if !out.is_empty()
            && !out.ends_with('\n')
            && out.chars().last().is_some_and(char::is_alphanumeric)
            && chunk.chars().next().is_some_and(char::is_alphanumeric)
        {
            out.push(' ');
        }
        out.push_str(chunk.as_str());
    }
    let trimmed = out.trim().to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

async fn download_media_items(
    client: &reqwest::Client,
    out_dir: &Path,
    items: &[(&str, &str)],
) -> Vec<FetchedProductMediaFile> {
    let mut saved = Vec::new();
    for (index, (kind, url)) in items.iter().enumerate() {
        log::info!(
            "product fetch media request kind={} index={} url={}",
            kind,
            index,
            safe_url_label(url)
        );
        let Ok(response) = client.get(*url).send().await else {
            log::warn!(
                "product fetch media request failed kind={} index={}",
                kind,
                index
            );
            continue;
        };
        if !response.status().is_success() {
            log::warn!(
                "product fetch media skipped kind={} index={} status={}",
                kind,
                index,
                response.status().as_u16()
            );
            continue;
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            .map(str::to_ascii_lowercase);
        let Ok(bytes) = response.bytes().await else {
            log::warn!(
                "product fetch media body failed kind={} index={}",
                kind,
                index
            );
            continue;
        };
        let max_bytes = if *kind == "video" {
            MAX_VIDEO_BYTES
        } else {
            MAX_IMAGE_BYTES
        };
        if bytes.len() > max_bytes {
            log::warn!(
                "product fetch media skipped kind={} index={} bytes={} max_bytes={}",
                kind,
                index,
                bytes.len(),
                max_bytes
            );
            continue;
        }
        let path = out_dir.join(format!(
            "{}_{index:03}{}",
            kind,
            file_extension(url, content_type.as_deref(), kind)
        ));
        if std::fs::write(path.as_path(), bytes.as_ref()).is_err() {
            log::warn!(
                "product fetch media write failed kind={} index={} path={}",
                kind,
                index,
                path.display()
            );
            continue;
        }
        log::info!(
            "product fetch media saved kind={} index={} bytes={} path={}",
            kind,
            index,
            bytes.len(),
            path.display()
        );
        saved.push(FetchedProductMediaFile {
            kind: (*kind).to_string(),
            path: path
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(path.as_path()))
                .to_string_lossy()
                .into_owned(),
            source_url: (*url).to_string(),
        });
    }
    saved
}

fn file_extension(url: &str, content_type: Option<&str>, kind: &str) -> String {
    let path = url
        .split('?')
        .next()
        .and_then(|value| value.rsplit('/').next())
        .unwrap_or("");
    if let Some((_, ext)) = path.rsplit_once('.') {
        if (1..=7).contains(&ext.len()) && ext.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return format!(".{}", ext.to_ascii_lowercase());
        }
    }
    match content_type.unwrap_or("") {
        ct if ct.contains("jpeg") || ct.contains("jpg") => ".jpg".to_string(),
        ct if ct.contains("png") => ".png".to_string(),
        ct if ct.contains("webp") => ".webp".to_string(),
        ct if ct.contains("mp4") => ".mp4".to_string(),
        ct if ct.contains("mpegurl") || ct.contains("m3u8") => ".m3u8".to_string(),
        _ if kind == "video" => ".mp4".to_string(),
        _ => ".jpg".to_string(),
    }
}

fn find_tags<'a>(html: &'a str, tag_name: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("<{tag_name}");
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = lower[pos..].find(needle.as_str()) {
        let start = pos + rel;
        let Some(end_rel) = lower[start..].find('>') else {
            break;
        };
        let end = start + end_rel + 1;
        out.push(&html[start..end]);
        pos = end;
    }
    out
}

fn find_script_text_by_type<'a>(html: &'a str, script_type: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(rel) = lower[pos..].find("<script") {
        let tag_start = pos + rel;
        let Some(open_end_rel) = lower[tag_start..].find('>') else {
            break;
        };
        let open_end = tag_start + open_end_rel + 1;
        let tag = &html[tag_start..open_end];
        let attrs = parse_attrs(tag);
        let matches_type = attrs
            .get("type")
            .map(|value| value.eq_ignore_ascii_case(script_type))
            .unwrap_or(false);
        let Some(close_rel) = lower[open_end..].find("</script>") else {
            break;
        };
        let close_start = open_end + close_rel;
        if matches_type {
            out.push(&html[open_end..close_start]);
        }
        pos = close_start + "</script>".len();
    }
    out
}

fn find_element_text(html: &str, tag_name: &str) -> Option<String> {
    let lower = html.to_ascii_lowercase();
    let open = format!("<{tag_name}");
    let start = lower.find(open.as_str())?;
    let content_start = start + lower[start..].find('>')? + 1;
    let close = format!("</{tag_name}>");
    let content_end = content_start + lower[content_start..].find(close.as_str())?;
    Some(html_unescape_basic(&html[content_start..content_end]))
}

fn parse_attrs(tag: &str) -> HashMap<String, String> {
    let mut attrs = HashMap::new();
    let bytes = tag.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] != b'<' {
        i += 1;
    }
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
        i += 1;
    }
    while i < bytes.len() {
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] == b'>' {
            break;
        }
        let key_start = i;
        while i < bytes.len()
            && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b'_' | b':' | b'.'))
        {
            i += 1;
        }
        if key_start == i {
            i += 1;
            continue;
        }
        let key = tag[key_start..i].to_ascii_lowercase();
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            attrs.insert(key, String::new());
            continue;
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < bytes.len() && (bytes[i] == b'"' || bytes[i] == b'\'') {
            let quote = bytes[i];
            i += 1;
            let value_start = i;
            while i < bytes.len() && bytes[i] != quote {
                i += 1;
            }
            let value = tag[value_start..i].to_string();
            if i < bytes.len() {
                i += 1;
            }
            value
        } else {
            let value_start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
                i += 1;
            }
            tag[value_start..i].to_string()
        };
        attrs.insert(key, html_unescape_basic(value.as_str()));
    }
    attrs
}

fn html_unescape_basic(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        description_from_product_model, gallery_image_urls_from_product_model, parse_product_html,
        parse_product_model_from_html, product_id_from_final_url,
        video_urls_from_product_model_videos,
    };
    use serde_json::json;

    #[test]
    fn parses_og_and_product_id_from_html() {
        let html = r#"
          <html><head>
            <meta content="Demo Product" property="og:title">
            <meta property="og:description" content="Short copy">
            <meta property="og:image" content="https://cdn.example/a.jpg">
          </head></html>
        "#;

        let product = parse_product_html(html, "https://shop.tiktok.com/view/product/12345?x=1");

        assert_eq!(product.name.as_deref(), Some("Demo Product"));
        assert_eq!(product.description.as_deref(), Some("Short copy"));
        assert_eq!(product.tiktok_shop_id.as_deref(), Some("12345"));
        assert_eq!(product.image_urls, vec!["https://cdn.example/a.jpg"]);
    }

    #[test]
    fn json_ld_fills_fallback_fields() {
        let html = r#"
          <script type="application/ld+json">
          {"@type":"Product","name":"JSON Product","description":"Long","image":["https://cdn/img.jpg"],"category":"Beauty","sku":"sku-7","offers":{"price":"12.5"}}
          </script>
        "#;

        let product = parse_product_html(html, "https://example.test/p");

        assert_eq!(product.name.as_deref(), Some("JSON Product"));
        assert_eq!(product.price, Some(12.5));
        assert_eq!(product.category.as_deref(), Some("Beauty"));
        assert_eq!(product.tiktok_shop_id.as_deref(), Some("sku-7"));
    }

    #[test]
    fn parses_loader_product_model_gallery_and_description() {
        let html = r#"
          <script type="application/json">{
            "loaderData": {
              "page": {
                "page_config": {
                  "components_map": [{
                    "component_data": {
                      "product_info": {
                        "product_model": {
                          "product_id": "p1",
                          "description": "[{\"type\":\"text\",\"text\":\"Line one\"},{\"type\":\"image\"},{\"type\":\"text\",\"text\":\"Line two\"}]",
                          "images": [
                            {"url_list":["https://cdn/a.jpg","https://cdn/a-small.jpg"]},
                            {"url_list":["https://cdn/b.webp"]}
                          ],
                          "videos": {"play_addr": {"url_list": ["https://cdn/v.mp4", "https://cdn/v.m3u8"]}}
                        }
                      }
                    }
                  }]
                }
              }
            }
          }</script>
        "#;

        let product_model = parse_product_model_from_html(html).expect("product model");

        assert_eq!(
            description_from_product_model(&product_model).as_deref(),
            Some("Line one\nLine two")
        );
        assert_eq!(
            gallery_image_urls_from_product_model(&product_model),
            vec!["https://cdn/a.jpg", "https://cdn/b.webp"]
        );
        assert_eq!(
            video_urls_from_product_model_videos(product_model.get("videos"), 8),
            vec!["https://cdn/v.mp4", "https://cdn/v.m3u8"]
        );
    }

    #[test]
    fn product_id_from_final_url_requires_digits() {
        assert_eq!(
            product_id_from_final_url("https://x/view/product/998877/foo").as_deref(),
            Some("998877")
        );
        assert_eq!(
            product_id_from_final_url("https://x/view/product/abc"),
            None
        );
    }

    #[test]
    fn description_accepts_array_value() {
        let product_model = json!({
            "description": [
                {"type": "text", "text": "A"},
                {"type": "text", "text": "B"}
            ]
        });

        assert_eq!(
            description_from_product_model(&product_model).as_deref(),
            Some("A B")
        );
    }
}
