CREATE TABLE IF NOT EXISTS product_embedding_vectors (
  id TEXT PRIMARY KEY,
  product_id INTEGER NOT NULL,
  modality TEXT NOT NULL CHECK (modality IN ('image', 'video', 'text')),
  image_path TEXT NOT NULL,
  source_url TEXT,
  product_name TEXT,
  product_text TEXT,
  product_description TEXT,
  embedding BLOB NOT NULL,
  embedding_dim INTEGER NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now', '+7 hours'))
);

CREATE INDEX IF NOT EXISTS idx_product_embedding_vectors_product
  ON product_embedding_vectors(product_id);

CREATE INDEX IF NOT EXISTS idx_product_embedding_vectors_modality
  ON product_embedding_vectors(modality, embedding_dim);
