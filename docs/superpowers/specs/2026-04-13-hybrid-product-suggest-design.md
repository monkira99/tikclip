# Hybrid Product Suggest — Design Spec

## Goal

Cải thiện chất lượng suggest-product bằng cách kết hợp **text hybrid search** (dense + BM25 sparse từ STT transcript) với **image search** hiện tại (frame embedding), sử dụng zvec native multi-vector query và Gemini asymmetric task types.

## Context

Luồng hiện tại chỉ dùng image: trích frame clip → Gemini multimodal embed → cosine nearest neighbor trong zvec → vote majority. Transcript STT đã có sẵn trong `ClipInfo` nhưng không được sử dụng. Product text (tên, mô tả) chỉ lưu SQLite, không được embed.

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  Index Flow (per product)            │
│                                                     │
│  Media files ──► Gemini multimodal embed ──► vector "embedding"  │
│                                                     │
│  Name + Desc ──► format_document_text() ──► Gemini  │
│               │  "title: {name} | text: {desc}"     │
│               │  task=RETRIEVAL_DOCUMENT (001)       │
│               ▼                                     │
│         vector "text_dense"                         │
│                                                     │
│  Name + Desc ──► BM25(encoding_type="document")     │
│               │  raw: "{name} {description}"         │
│               ▼                                     │
│         vector "text_sparse"                        │
│                                                     │
│  All vectors upserted into single zvec collection   │
│  "product_media"                                    │
└─────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────┐
│              Suggest Flow (per clip)                 │
│                                                     │
│  Step 1: Text Hybrid Search (if transcript exists)  │
│  ┌───────────────────────────────────────────┐      │
│  │ transcript ──► format_query_text()        │      │
│  │             │  "task: search result |     │      │
│  │             │   query: {transcript}"      │      │
│  │             │  task=RETRIEVAL_QUERY (001)  │      │
│  │             ▼                             │      │
│  │       Gemini embed → text_dense query     │      │
│  │                                           │      │
│  │ transcript ──► BM25(encoding_type="query")│      │
│  │             ▼                             │      │
│  │       text_sparse query                   │      │
│  │                                           │      │
│  │ zvec multi-vector query:                  │      │
│  │   [text_dense, text_sparse]               │      │
│  │   + RrfReRanker(topn=5)                   │      │
│  │   + filter="product_text IS NOT NULL"     │      │
│  │                                           │      │
│  │ → text_results: [(product_id, score)]     │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  Step 2: Image Search (existing flow, unchanged)    │
│  ┌───────────────────────────────────────────┐      │
│  │ frames ──► Gemini multimodal embed        │      │
│  │         ──► zvec query vector "embedding" │      │
│  │         ──► vote majority / tiebreak      │      │
│  │ → image_result: (product_id, score)       │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  Step 3: Weighted Score Fusion                      │
│  ┌───────────────────────────────────────────┐      │
│  │ normalize scores to [0, 1]                │      │
│  │ final = w_image * img + w_text * txt      │      │
│  │ defaults: w_image=0.6, w_text=0.4         │      │
│  │ pick best product > threshold             │      │
│  └───────────────────────────────────────────┘      │
│                                                     │
│  Fallback: no transcript → image-only (current)     │
└─────────────────────────────────────────────────────┘
```

## 1. Schema Changes — collection `product_media`

Extend existing zvec collection schema with new fields and vectors:

### New fields

| Field | DataType | Nullable | Purpose |
|-------|----------|----------|---------|
| `product_text` | STRING | true | Raw combined text `"{name} {description}"` for BM25 corpus rebuild |
| `product_description` | STRING | true | Product description stored for reference |

### New vectors

| Vector | DataType | Index | Purpose |
|--------|----------|-------|---------|
| `text_dense` | VECTOR_FP32 (dim=settings) | HNSW COSINE | Gemini text embedding (asymmetric retrieval) |
| `text_sparse` | SPARSE_VECTOR_FP32 | — | BM25 sparse vector |

### Existing (unchanged)

| Vector/Field | Note |
|-------------|------|
| `embedding` (VECTOR_FP32) | Image/video dense vector |
| `product_id` (INT64) | Product FK |
| `image_path` (STRING) | Media file path |
| `source_url` (STRING) | Optional source URL |
| `product_name` (STRING) | Product name |
| `modality` (STRING) | "image" / "video" / "text" |

### Document types in collection

- **Media docs** `p{product_id}_{i}` — vector `embedding` populated, `text_dense`/`text_sparse` null
- **Text docs** `t{product_id}` — vectors `text_dense` + `text_sparse` populated, `embedding` null, `modality="text"`

**Migration:** Requires collection rebuild (schema change adds vector fields). Existing `product_media` folder deleted and re-indexed. User triggers via "Rebuild Index" or automatic on first index after upgrade.

## 2. Gemini Task Types — Asymmetric Embedding

Replace hardcoded `_EMBED_TASK_TYPE = "SEMANTIC_SIMILARITY"` with role-aware formatting.

### For `gemini-embedding-2-preview` (prefix-based, no `task_type` field)

| Purpose | Format |
|---------|--------|
| Index product text | `"title: {name} \| text: {description}"` |
| Query transcript | `"task: search result \| query: {transcript}"` |
| Embed media | No prefix needed (multimodal, no `task_type`) |

### For `gemini-embedding-001` (field-based `task_type`)

| Purpose | `task_type` value |
|---------|-------------------|
| Index product text | `RETRIEVAL_DOCUMENT` |
| Query transcript | `RETRIEVAL_QUERY` |
| Embed media | `SEMANTIC_SIMILARITY` |

### API changes in `gemini.py`

`embed_text()` gains a `role` parameter: `"query"` or `"document"`.
- If model is `embedding-2*`: format text with prefix, do NOT set `task_type` in config
- If model is `embedding-001`: set `task_type` field in config, pass raw text

`embed_file()`:
- If model is `embedding-2*`: remove `task_type` from config (not supported)
- If model is `embedding-001`: keep `task_type=SEMANTIC_SIMILARITY`

## 3. Index Flow — per product save/update

### Step 1 — Media (existing, minor change)

Embed each media file → upsert docs `p{product_id}_{i}` with vector `embedding`.
Change: `embed_file()` no longer passes `task_type` for embedding-2 model.

### Step 2 — Text (new)

1. Build formatted text:
   - Dense input: `format_document_text(name, description)` → `"title: {name} | text: {description}"`
   - Sparse input: `f"{name} {description}"` (raw for BM25 tokenizer)
2. Embed dense: `gemini.embed_text(role="document")` → `text_dense` vector
3. Embed sparse: `BM25EmbeddingFunction(corpus=..., encoding_type="document").embed(raw_text)` → `text_sparse` vector
4. Upsert doc `t{product_id}` with both vectors + fields `product_text`, `product_name`, `product_description`, `product_id`, `modality="text"`

### Step 3 — Delete product

Delete `p{product_id}_*` (media) + `t{product_id}` (text). Invalidate BM25 cache.

## 4. BM25 Corpus Management — Lazy Rebuild

BM25 requires a corpus to compute IDF weights. Changes to product catalog invalidate the corpus.

### Strategy

- **In-memory cache:** `_bm25_instance: BM25EmbeddingFunction | None`, `_bm25_corpus_size: int`
- **On index product:** After upsert text doc, load all `product_text` fields from collection → rebuild BM25 → cache
- **On delete product:** Invalidate cache (`_bm25_instance = None`)
- **Before suggest query:** Check `_bm25_instance is None` or count text docs != `_bm25_corpus_size` → rebuild if stale
- **Corpus source:** Query all docs where `product_text IS NOT NULL`, extract `product_text` field values

### Performance

Product catalogs in TikTok seller context are typically 10–500 items. BM25 rebuild on 500 texts is sub-millisecond. No concern.

## 5. Suggest Flow — per clip

### Input change

`ClipSuggestProductRequest` adds optional `transcript_text: str | None`.

### Step 1 — Text Hybrid Search (if transcript_text is not empty)

1. Format query: `format_query_text(transcript)` → `"task: search result | query: {transcript}"` (embedding-2) or raw + `task_type=RETRIEVAL_QUERY` (001)
2. Embed dense: `gemini.embed_text(role="query")` → dense query vector
3. Embed sparse: `BM25EmbeddingFunction(corpus=cached, encoding_type="query").embed(transcript)` → sparse query vector
4. Multi-vector query:
   ```python
   coll.query(
       vectors=[
           VectorQuery("text_dense", vector=dense_vec),
           VectorQuery("text_sparse", vector=sparse_vec),
       ],
       reranker=RrfReRanker(topn=5),
       filter="product_text IS NOT NULL",
       topk=5,
       output_fields=["product_id", "product_name", "product_text"],
   )
   ```
5. Result: `text_results: list[(product_id, rrf_score)]`

### Step 2 — Image Search (existing flow, unchanged)

Extract frames → embed each → query `embedding` vector → vote majority / min distance tiebreak.
Result: `image_results: list[(product_id, image_score)]` from vote aggregation.

### Step 3 — Weighted Score Fusion

1. Normalize scores to `[0, 1]`:
   - Image: cosine distance → `1.0 - distance` (higher = better match)
   - Text RRF: already a relevance score, normalize by `score / max_score` in result set
2. For each candidate product_id appearing in either result set:
   ```
   final_score = w_image * norm_image_score + w_text * norm_text_score
   ```
   If product only appears in one set, the missing signal contributes 0.
3. Pick product with highest `final_score`
4. Compare against threshold `auto_tag_clip_max_score`
5. Default weights: `suggest_weight_image = 0.6`, `suggest_weight_text = 0.4`

### Fallback

No transcript (STT disabled, silent clip, or empty text) → skip Step 1, run image-only as current behavior. Zero breaking changes.

## 6. New Settings

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `suggest_weight_image` | float | 0.6 | Weight for image search score in fusion |
| `suggest_weight_text` | float | 0.4 | Weight for text search score in fusion |

Exposed through existing settings flow: Tauri env → sidecar config → UI settings page.

## 7. Files to Change

### Sidecar (Python)

| File | Change |
|------|--------|
| `src/embeddings/gemini.py` | Add `role` param to `embed_text()`, model-aware task type / prefix logic, remove hardcoded `SEMANTIC_SIMILARITY` for `embed_file()` on embedding-2 |
| `src/embeddings/product_vector.py` | Extend schema (new fields + vectors), add text index/query functions, BM25 corpus cache, schema migration/rebuild logic |
| `src/embeddings/clip_product_suggest.py` | Accept `transcript_text`, add text hybrid search step, weighted fusion logic |
| `src/models/schemas.py` | `ClipSuggestProductRequest` add `transcript_text`, response add text search fields |
| `src/routes/clips.py` | Pass `transcript_text` through to suggest function |
| `src/config.py` | Add `suggest_weight_image`, `suggest_weight_text` |

### Frontend (TypeScript)

| File | Change |
|------|--------|
| `src/lib/api.ts` | `suggestProductForClip()` send `transcript_text` |
| `src/components/layout/app-shell.tsx` | Pass clip `transcript_text` to suggest API call |

### Tauri (Rust)

| File | Change |
|------|--------|
| `src-tauri/src/sidecar_env.rs` | Forward new weight settings to sidecar env |

## 8. Out of Scope

- UI for manual text search / hybrid search results display
- Multi-language BM25 tokenizer optimization
- Gemini embedding model auto-selection
- Re-ranking with LLM (Gemini generative)
