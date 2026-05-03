use base64::Engine;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use super::common::{bool_setting, int_setting, resolve_storage_media_path, string_setting};

const TEXT_PATH_PLACEHOLDER: &str = "__text__";
const MAX_EMBED_BYTES_IMAGE: u64 = 20 * 1024 * 1024;
const MAX_EMBED_BYTES_VIDEO: u64 = 80 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ProductEmbeddingMediaItem {
    pub kind: String,
    pub path: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct IndexProductEmbeddingsInput {
    pub product_id: i64,
    pub product_name: String,
    pub product_description: String,
    pub items: Vec<ProductEmbeddingMediaItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexProductEmbeddingsResponse {
    pub indexed: i64,
    pub skipped: i64,
    pub errors: Vec<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DeleteProductEmbeddingsInput {
    pub product_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProductEmbeddingsResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProductEmbeddingSearchHit {
    pub product_id: i64,
    pub score: f64,
    pub image_path: String,
    pub source_url: Option<String>,
    pub product_name: Option<String>,
    pub modality: Option<String>,
    pub product_text: Option<String>,
    pub product_description: Option<String>,
}

#[derive(Debug, Clone)]
struct ProductVectorSettings {
    enabled: bool,
    api_key: Option<String>,
    model: String,
    dim: usize,
    media_suffix: String,
}

#[derive(Debug, Clone)]
struct StoredVector {
    product_id: i64,
    image_path: String,
    source_url: Option<String>,
    product_name: Option<String>,
    modality: Option<String>,
    product_text: Option<String>,
    product_description: Option<String>,
    vector: Vec<f32>,
}

struct VectorUpsert {
    id: String,
    product_id: i64,
    modality: String,
    image_path: String,
    source_url: Option<String>,
    product_name: Option<String>,
    product_text: Option<String>,
    product_description: Option<String>,
    vector: Vec<f32>,
}

impl ProductVectorSettings {
    fn from_conn(conn: &Connection) -> Result<Self, String> {
        Ok(Self {
            enabled: bool_setting(conn, "product_vector_enabled", false)?,
            api_key: string_setting(conn, "gemini_api_key")?,
            model: string_setting(conn, "gemini_embedding_model")?
                .unwrap_or_else(|| "gemini-embedding-2-preview".to_string()),
            dim: int_setting(conn, "gemini_embedding_dimensions", 1536)?.clamp(1, 8192) as usize,
            media_suffix: string_setting(conn, "product_media_embed_suffix")?
                .unwrap_or_else(|| "đang được mặc hoặc cầm trên tay giới thiệu".to_string()),
        })
    }
}

pub fn index_product_embeddings_with_db_lock(
    db: &Mutex<Connection>,
    storage_root: &Path,
    input: &IndexProductEmbeddingsInput,
) -> Result<IndexProductEmbeddingsResponse, String> {
    log::info!(
        "product embedding index requested product_id={} media_items={}",
        input.product_id,
        input.items.len()
    );
    if input.product_id < 1 {
        log::warn!(
            "product embedding index rejected invalid product_id={}",
            input.product_id
        );
        return Err("product_id must be positive".to_string());
    }
    let settings = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        ProductVectorSettings::from_conn(&conn)?
    };
    if !settings.enabled {
        log::info!(
            "product embedding index skipped product_id={} reason=disabled",
            input.product_id
        );
        return Ok(IndexProductEmbeddingsResponse {
            skipped: i64::try_from(input.items.len()).unwrap_or(i64::MAX),
            message: Some("Product vector indexing is disabled in settings".to_string()),
            ..IndexProductEmbeddingsResponse::default()
        });
    }
    if settings.api_key.is_none() {
        log::info!(
            "product embedding index skipped product_id={} reason=missing_gemini_api_key",
            input.product_id
        );
        return Ok(IndexProductEmbeddingsResponse {
            skipped: i64::try_from(input.items.len()).unwrap_or(i64::MAX),
            message: Some("Gemini API key is not configured".to_string()),
            ..IndexProductEmbeddingsResponse::default()
        });
    }

    let (rows, response) = build_index_rows(&settings, storage_root, input)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    delete_media_docs_for_product(&conn, input.product_id)?;
    commit_index_rows(&conn, input.product_id, &rows)?;
    log::info!(
        "product embedding index completed product_id={} indexed={} skipped={} errors={} rows_committed={}",
        input.product_id,
        response.indexed,
        response.skipped,
        response.errors.len(),
        rows.len()
    );
    Ok(response)
}

fn build_index_rows(
    settings: &ProductVectorSettings,
    storage_root: &Path,
    input: &IndexProductEmbeddingsInput,
) -> Result<(Vec<VectorUpsert>, IndexProductEmbeddingsResponse), String> {
    let api_key = settings
        .api_key
        .as_deref()
        .ok_or_else(|| "Gemini API key is not configured".to_string())?;
    let mut indexed = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();
    let mut rows = Vec::new();
    let product_name = input.product_name.trim();
    let product_description = input.product_description.trim();

    for (index, item) in input.items.iter().enumerate() {
        log::info!(
            "product embedding media item started product_id={} index={} kind={}",
            input.product_id,
            index,
            item.kind
        );
        if item.kind != "image" && item.kind != "video" {
            log::info!(
                "product embedding media item skipped product_id={} index={} reason=unsupported_kind kind={}",
                input.product_id,
                index,
                item.kind
            );
            skipped += 1;
            continue;
        }
        let path = match resolve_storage_media_path(storage_root, item.path.as_str()) {
            Ok(path) => path,
            Err(err) => {
                log::warn!(
                    "product embedding media item skipped product_id={} index={} reason=path_error error={}",
                    input.product_id,
                    index,
                    err
                );
                errors.push(format!("{}: {err}", item.path));
                skipped += 1;
                continue;
            }
        };
        if path.extension().is_some_and(|ext| ext == "m3u8") {
            log::info!(
                "product embedding media item skipped product_id={} index={} reason=m3u8 path={}",
                input.product_id,
                index,
                path.display()
            );
            skipped += 1;
            continue;
        }
        let caption = catalog_caption(product_name, settings.media_suffix.as_str());
        let vector = match embed_file(
            api_key,
            settings,
            &path,
            item.kind.as_str(),
            caption.as_deref(),
        ) {
            Ok(vector) => vector,
            Err(err) => {
                log::warn!(
                    "product embedding media item failed product_id={} index={} path={} error={}",
                    input.product_id,
                    index,
                    path.display(),
                    err
                );
                errors.push(format!("{}: {err}", item.path));
                skipped += 1;
                continue;
            }
        };
        if vector.len() != settings.dim {
            log::warn!(
                "product embedding media item skipped product_id={} index={} reason=dimension_mismatch got={} expected={}",
                input.product_id,
                index,
                vector.len(),
                settings.dim
            );
            errors.push(format!(
                "{}: embedding length {} != configured {}",
                item.path,
                vector.len(),
                settings.dim
            ));
            skipped += 1;
            continue;
        }
        let product_description_field =
            empty_as_none(product_description).map(|s| truncate_chars(s.as_str(), 8000));
        rows.push(VectorUpsert {
            id: format!("p{}_{}", input.product_id, index),
            product_id: input.product_id,
            modality: item.kind.clone(),
            image_path: path.to_string_lossy().into_owned(),
            source_url: empty_as_none(item.source_url.as_str()),
            product_name: empty_as_none(product_name),
            product_text: None,
            product_description: product_description_field,
            vector,
        });
        indexed += 1;
        log::info!(
            "product embedding media item indexed product_id={} index={} kind={} dim={}",
            input.product_id,
            index,
            item.kind,
            settings.dim
        );
    }

    if !product_name.is_empty() || !product_description.is_empty() {
        log::info!(
            "product text embedding started product_id={} name_present={} description_present={}",
            input.product_id,
            !product_name.is_empty(),
            !product_description.is_empty()
        );
        match build_product_text_row(
            api_key,
            settings,
            input.product_id,
            product_name,
            product_description,
        ) {
            Ok(Some(row)) => {
                rows.push(row);
                indexed += 1;
                log::info!(
                    "product text embedding indexed product_id={}",
                    input.product_id
                );
            }
            Ok(None) => {}
            Err(err) => {
                log::warn!(
                    "product text embedding failed product_id={} error={}",
                    input.product_id,
                    err
                );
                errors.push(err);
            }
        }
    }

    Ok((
        rows,
        IndexProductEmbeddingsResponse {
            indexed,
            skipped,
            errors,
            message: None,
        },
    ))
}

pub fn delete_product_embeddings(
    conn: &Connection,
    input: &DeleteProductEmbeddingsInput,
) -> Result<DeleteProductEmbeddingsResponse, String> {
    let deleted = conn
        .execute(
            "DELETE FROM product_embedding_vectors WHERE product_id = ?1",
            [input.product_id],
        )
        .map_err(|e| e.to_string())?;
    log::info!(
        "product embeddings deleted product_id={} rows={}",
        input.product_id,
        deleted
    );
    Ok(DeleteProductEmbeddingsResponse { ok: true })
}

pub fn search_by_text(
    conn: &Connection,
    query: &str,
    top_k: i64,
) -> Result<Vec<ProductEmbeddingSearchHit>, String> {
    let settings = require_search_settings(conn)?;
    log::info!(
        "product embedding text search started top_k={} query_chars={}",
        top_k,
        query.chars().count()
    );
    let vector = embed_text(
        settings.api_key.as_deref().unwrap_or(""),
        &settings,
        query,
        "query",
        None,
    )?;
    let docs = load_all_vectors(conn, Some("text"), 100_000)?;
    let hits = top_dense_hits(&vector, docs, top_k, ScoreMode::Similarity);
    log::info!(
        "product embedding text search completed hits={}",
        hits.len()
    );
    Ok(hits)
}

pub fn search_by_text_with_db_lock(
    db: &Mutex<Connection>,
    query: &str,
    top_k: i64,
) -> Result<Vec<ProductEmbeddingSearchHit>, String> {
    let (settings, docs) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let settings = require_search_settings(&conn)?;
        let docs = load_all_vectors(&conn, Some("text"), 100_000)?;
        (settings, docs)
    };
    let vector = embed_text(
        settings.api_key.as_deref().unwrap_or(""),
        &settings,
        query,
        "query",
        None,
    )?;
    let docs_len = docs.len();
    let hits = top_dense_hits(&vector, docs, top_k, ScoreMode::Similarity);
    log::info!(
        "product embedding text search completed hits={} docs={}",
        hits.len(),
        docs_len
    );
    Ok(hits)
}

pub fn search_by_media_path(
    conn: &Connection,
    storage_root: &Path,
    media_path: &str,
    kind: &str,
    top_k: i64,
    companion_text: Option<&str>,
) -> Result<Vec<ProductEmbeddingSearchHit>, String> {
    let settings = require_search_settings(conn)?;
    let fs_path = resolve_storage_media_path(storage_root, media_path)?;
    log::info!(
        "product embedding media search started kind={} top_k={} path={}",
        kind,
        top_k,
        fs_path.display()
    );
    let vector = embed_file(
        settings.api_key.as_deref().unwrap_or(""),
        &settings,
        &fs_path,
        kind,
        companion_text,
    )?;
    let docs = load_media_vectors(conn, 100_000)?;
    let hits = top_dense_hits(&vector, docs, top_k, ScoreMode::Distance);
    log::info!(
        "product embedding media search completed hits={}",
        hits.len()
    );
    Ok(hits)
}

pub fn search_by_media_path_with_db_lock(
    db: &Mutex<Connection>,
    storage_root: &Path,
    media_path: &str,
    kind: &str,
    top_k: i64,
    companion_text: Option<&str>,
) -> Result<Vec<ProductEmbeddingSearchHit>, String> {
    let fs_path = resolve_storage_media_path(storage_root, media_path)?;
    log::info!(
        "product embedding media search started kind={} top_k={} path={}",
        kind,
        top_k,
        fs_path.display()
    );
    let (settings, docs) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        let settings = require_search_settings(&conn)?;
        let docs = load_media_vectors(&conn, 100_000)?;
        (settings, docs)
    };
    let vector = embed_file(
        settings.api_key.as_deref().unwrap_or(""),
        &settings,
        &fs_path,
        kind,
        companion_text,
    )?;
    let docs_len = docs.len();
    let hits = top_dense_hits(&vector, docs, top_k, ScoreMode::Distance);
    log::info!(
        "product embedding media search completed hits={} docs={}",
        hits.len(),
        docs_len
    );
    Ok(hits)
}

fn require_search_settings(conn: &Connection) -> Result<ProductVectorSettings, String> {
    let settings = ProductVectorSettings::from_conn(conn)?;
    if !settings.enabled {
        return Err("Product vector search is disabled in settings".to_string());
    }
    if settings.api_key.is_none() {
        return Err("Gemini API key is not configured".to_string());
    }
    Ok(settings)
}

fn build_product_text_row(
    api_key: &str,
    settings: &ProductVectorSettings,
    product_id: i64,
    name: &str,
    description: &str,
) -> Result<Option<VectorUpsert>, String> {
    let raw_text = format!("{name} {description}").trim().to_string();
    if raw_text.is_empty() {
        return Ok(None);
    }
    let vector = embed_text(
        api_key,
        settings,
        raw_text.as_str(),
        "document",
        empty_as_none(name).as_deref(),
    )?;
    if vector.len() != settings.dim {
        return Err(format!(
            "Text embedding length {} != configured {}",
            vector.len(),
            settings.dim
        ));
    }
    Ok(Some(VectorUpsert {
        id: format!("t{product_id}"),
        product_id,
        modality: "text".to_string(),
        image_path: TEXT_PATH_PLACEHOLDER.to_string(),
        source_url: None,
        product_name: empty_as_none(name),
        product_text: Some(raw_text),
        product_description: empty_as_none(description),
        vector,
    }))
}

fn commit_index_rows(
    conn: &Connection,
    product_id: i64,
    rows: &[VectorUpsert],
) -> Result<(), String> {
    if rows.iter().any(|row| row.modality == "text") {
        conn.execute(
            "DELETE FROM product_embedding_vectors WHERE product_id = ?1 AND modality = 'text'",
            [product_id],
        )
        .map_err(|e| e.to_string())?;
    }
    for row in rows {
        upsert_vector(conn, row)?;
    }
    Ok(())
}

fn upsert_vector(conn: &Connection, row: &VectorUpsert) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO product_embedding_vectors (
            id, product_id, modality, image_path, source_url, product_name,
            product_text, product_description, embedding, embedding_dim, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, datetime('now', '+7 hours'))",
        params![
            row.id,
            row.product_id,
            row.modality,
            row.image_path,
            row.source_url,
            row.product_name,
            row.product_text,
            row.product_description,
            encode_f32_blob(&row.vector),
            i64::try_from(row.vector.len()).map_err(|e| e.to_string())?,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn delete_media_docs_for_product(conn: &Connection, product_id: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM product_embedding_vectors WHERE product_id = ?1 AND modality IN ('image', 'video')",
        [product_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_media_vectors(conn: &Connection, limit: i64) -> Result<Vec<StoredVector>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT product_id, image_path, source_url, product_name, modality, product_text, product_description, embedding
             FROM product_embedding_vectors
             WHERE modality IN ('image', 'video')
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    query_vectors(&mut stmt, [limit])
}

fn load_all_vectors(
    conn: &Connection,
    modality: Option<&str>,
    limit: i64,
) -> Result<Vec<StoredVector>, String> {
    if let Some(modality) = modality {
        let mut stmt = conn
            .prepare(
                "SELECT product_id, image_path, source_url, product_name, modality, product_text, product_description, embedding
                 FROM product_embedding_vectors
                 WHERE modality = ?1
                 LIMIT ?2",
            )
            .map_err(|e| e.to_string())?;
        return query_vectors(&mut stmt, params![modality, limit]);
    }
    let mut stmt = conn
        .prepare(
            "SELECT product_id, image_path, source_url, product_name, modality, product_text, product_description, embedding
             FROM product_embedding_vectors
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    query_vectors(&mut stmt, [limit])
}

fn query_vectors<P>(
    stmt: &mut rusqlite::Statement<'_>,
    params: P,
) -> Result<Vec<StoredVector>, String>
where
    P: rusqlite::Params,
{
    let rows = stmt
        .query_map(params, |row| {
            let blob: Vec<u8> = row.get(7)?;
            Ok(StoredVector {
                product_id: row.get(0)?,
                image_path: row.get(1)?,
                source_url: row.get(2)?,
                product_name: row.get(3)?,
                modality: row.get(4)?,
                product_text: row.get(5)?,
                product_description: row.get(6)?,
                vector: decode_f32_blob(&blob).unwrap_or_default(),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows {
        let vector = row.map_err(|e| e.to_string())?;
        if !vector.vector.is_empty() {
            out.push(vector);
        }
    }
    Ok(out)
}

enum ScoreMode {
    Similarity,
    Distance,
}

fn top_dense_hits(
    query: &[f32],
    docs: Vec<StoredVector>,
    top_k: i64,
    mode: ScoreMode,
) -> Vec<ProductEmbeddingSearchHit> {
    let mut hits: Vec<ProductEmbeddingSearchHit> = docs
        .into_iter()
        .filter(|doc| doc.vector.len() == query.len())
        .filter_map(|doc| {
            let sim = normalized_cosine(query, &doc.vector)?;
            let score = match mode {
                ScoreMode::Similarity => sim,
                ScoreMode::Distance => 1.0 - sim,
            };
            Some(ProductEmbeddingSearchHit {
                product_id: doc.product_id,
                score,
                image_path: doc.image_path,
                source_url: doc.source_url,
                product_name: doc.product_name,
                modality: doc.modality,
                product_text: doc.product_text,
                product_description: doc.product_description,
            })
        })
        .collect();
    match mode {
        ScoreMode::Similarity => hits.sort_by(|a, b| b.score.total_cmp(&a.score)),
        ScoreMode::Distance => hits.sort_by(|a, b| a.score.total_cmp(&b.score)),
    }
    hits.truncate(top_k.max(1) as usize);
    hits
}

fn normalized_cosine(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let mut dot = 0.0_f64;
    let mut an = 0.0_f64;
    let mut bn = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = f64::from(*x);
        let yf = f64::from(*y);
        dot += xf * yf;
        an += xf * xf;
        bn += yf * yf;
    }
    if an <= f64::EPSILON || bn <= f64::EPSILON {
        return None;
    }
    let cosine = dot / (an.sqrt() * bn.sqrt());
    Some(((cosine + 1.0) / 2.0).clamp(0.0, 1.0))
}

fn embed_text(
    api_key: &str,
    settings: &ProductVectorSettings,
    text: &str,
    role: &str,
    title: Option<&str>,
) -> Result<Vec<f32>, String> {
    let raw = text.trim();
    if role == "query" && raw.is_empty() {
        return Err("Empty text for embedding".to_string());
    }
    if role == "document" && raw.is_empty() && title.unwrap_or("").trim().is_empty() {
        return Err("Empty document for embedding".to_string());
    }
    let (content_text, task_type) = embed_text_content(settings.model.as_str(), raw, role, title);
    let request = serde_json::json!({
        "content": { "parts": [{ "text": content_text }] },
        "outputDimensionality": settings.dim,
    });
    let request = if let Some(task_type) = task_type {
        let mut object = request.as_object().cloned().unwrap_or_default();
        object.insert("taskType".to_string(), serde_json::json!(task_type));
        serde_json::Value::Object(object)
    } else {
        request
    };
    call_gemini_embed(api_key, settings.model.as_str(), &request)
}

fn embed_file(
    api_key: &str,
    settings: &ProductVectorSettings,
    path: &Path,
    kind: &str,
    companion_text: Option<&str>,
) -> Result<Vec<f32>, String> {
    if !path.is_file() {
        return Err(format!("Media file not found: {}", path.display()));
    }
    let max_bytes = if kind == "video" {
        MAX_EMBED_BYTES_VIDEO
    } else {
        MAX_EMBED_BYTES_IMAGE
    };
    let size = path.metadata().map_err(|e| e.to_string())?.len();
    if size > max_bytes {
        return Err(format!(
            "File too large for embedding ({size} bytes): {}",
            path.display()
        ));
    }
    let media_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let inline_data = serde_json::json!({
        "mimeType": mime_for_path(path, kind),
        "data": base64::engine::general_purpose::STANDARD.encode(media_bytes),
    });
    let mut parts = Vec::new();
    if let Some(text) = companion_text.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(serde_json::json!({ "text": text }));
    }
    parts.push(serde_json::json!({ "inlineData": inline_data }));
    let mut request = serde_json::json!({
        "content": { "parts": parts },
        "outputDimensionality": settings.dim,
    });
    if !is_embedding_v2(settings.model.as_str()) {
        request["taskType"] = serde_json::json!("SEMANTIC_SIMILARITY");
    }
    call_gemini_embed(api_key, settings.model.as_str(), &request)
}

fn call_gemini_embed(
    api_key: &str,
    model: &str,
    body: &serde_json::Value,
) -> Result<Vec<f32>, String> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/{}:embedContent",
        gemini_model_path(model)
    );
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(url)
        .header("x-goog-api-key", api_key)
        .json(body)
        .send()
        .map_err(|e| format!("Gemini embedding request failed: {e}"))?;
    let status = response.status();
    let text = response.text().map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("Gemini embedding request failed: {status}: {text}"));
    }
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    embedding_values_from_json(&json)
}

fn embedding_values_from_json(json: &serde_json::Value) -> Result<Vec<f32>, String> {
    let values = json
        .get("embedding")
        .and_then(|embedding| embedding.get("values"))
        .or_else(|| {
            json.get("embeddings")
                .and_then(|v| v.as_array())
                .and_then(|embeddings| embeddings.first())
                .and_then(|first| first.get("values"))
        })
        .and_then(|values| values.as_array())
        .ok_or_else(|| "Gemini embed_content returned no embedding values".to_string())?;
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let number = value
            .as_f64()
            .ok_or_else(|| "Gemini embedding contained a non-number value".to_string())?;
        out.push(number as f32);
    }
    if out.is_empty() {
        return Err("Gemini embedding has empty values".to_string());
    }
    Ok(out)
}

fn embed_text_content(
    model: &str,
    raw_text: &str,
    role: &str,
    title: Option<&str>,
) -> (String, Option<&'static str>) {
    if is_embedding_v2(model) {
        if role == "query" {
            return (format!("task: search result | query: {raw_text}"), None);
        }
        let title = title.unwrap_or("").trim();
        let title = if title.is_empty() { "none" } else { title };
        let body = if raw_text.is_empty() {
            "none"
        } else {
            raw_text
        };
        return (format!("title: {title} | text: {body}"), None);
    }
    if role == "query" {
        return (raw_text.to_string(), Some("RETRIEVAL_QUERY"));
    }
    let body = if raw_text.trim().is_empty() {
        "none"
    } else {
        raw_text
    };
    (body.to_string(), Some("RETRIEVAL_DOCUMENT"))
}

fn is_embedding_v2(model: &str) -> bool {
    model.to_ascii_lowercase().contains("embedding-2")
}

fn gemini_model_path(model: &str) -> String {
    let trimmed = model.trim();
    if trimmed.starts_with("models/") || trimmed.starts_with("tunedModels/") {
        trimmed.to_string()
    } else {
        format!("models/{trimmed}")
    }
}

fn catalog_caption(product_name: &str, suffix: &str) -> Option<String> {
    let name = product_name.trim();
    let suffix = suffix.trim();
    match (name.is_empty(), suffix.is_empty()) {
        (true, _) => None,
        (false, true) => Some(name.to_string()),
        (false, false) => Some(format!("{name} {suffix}")),
    }
}

fn mime_for_path(path: &Path, kind: &str) -> &'static str {
    let suffix = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if kind == "video" {
        if suffix == "mov" {
            return "video/quicktime";
        }
        return "video/mp4";
    }
    match suffix.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "application/octet-stream",
    }
}

fn encode_f32_blob(values: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(values.len() * 4);
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

fn decode_f32_blob(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
        return Err("Invalid vector blob length".to_string());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect())
}

fn empty_as_none(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        decode_f32_blob, embed_text_content, embedding_values_from_json, encode_f32_blob,
        gemini_model_path, normalized_cosine, top_dense_hits, ProductEmbeddingSearchHit, ScoreMode,
        StoredVector,
    };

    #[test]
    fn f32_blob_round_trips_values() {
        let values = vec![0.25, -1.0, 3.5];
        let decoded = decode_f32_blob(&encode_f32_blob(&values)).expect("decode");
        assert_eq!(decoded, values);
    }

    #[test]
    fn normalized_cosine_maps_identical_vectors_to_one() {
        assert_eq!(normalized_cosine(&[1.0, 0.0], &[1.0, 0.0]), Some(1.0));
    }

    #[test]
    fn media_hits_sort_by_distance_ascending() {
        let docs = vec![
            StoredVector {
                product_id: 1,
                image_path: "a.jpg".to_string(),
                source_url: None,
                product_name: None,
                modality: Some("image".to_string()),
                product_text: None,
                product_description: None,
                vector: vec![1.0, 0.0],
            },
            StoredVector {
                product_id: 2,
                image_path: "b.jpg".to_string(),
                source_url: None,
                product_name: None,
                modality: Some("image".to_string()),
                product_text: None,
                product_description: None,
                vector: vec![0.0, 1.0],
            },
        ];
        let hits: Vec<ProductEmbeddingSearchHit> =
            top_dense_hits(&[1.0, 0.0], docs, 2, ScoreMode::Distance);
        assert_eq!(hits[0].product_id, 1);
        assert!(hits[0].score < hits[1].score);
    }

    #[test]
    fn embedding_two_text_content_uses_prompt_prefix_without_task_type() {
        let (content, task) =
            embed_text_content("gemini-embedding-2-preview", "red bag", "query", None);
        assert_eq!(content, "task: search result | query: red bag");
        assert_eq!(task, None);
    }

    #[test]
    fn gemini_rest_embedding_response_parses_singular_embedding_shape() {
        let json = serde_json::json!({
            "embedding": {
                "values": [0.1, -0.2]
            }
        });
        let values = embedding_values_from_json(&json).expect("values");
        assert_eq!(values, vec![0.1_f32, -0.2_f32]);
    }

    #[test]
    fn gemini_model_path_accepts_plain_or_resource_name() {
        assert_eq!(
            gemini_model_path("gemini-embedding-001"),
            "models/gemini-embedding-001"
        );
        assert_eq!(
            gemini_model_path("models/gemini-embedding-001"),
            "models/gemini-embedding-001"
        );
    }
}
