use crate::db::models::Product;
use crate::time_hcm::SQL_NOW_HCM;
use crate::AppState;
use rusqlite::{params, Row};
use serde::Deserialize;
use tauri::State;

fn map_product_row(row: &Row) -> rusqlite::Result<Product> {
    Ok(Product {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        sku: row.get(3)?,
        image_url: row.get(4)?,
        tiktok_shop_id: row.get(5)?,
        tiktok_url: row.get(6)?,
        price: row.get(7)?,
        category: row.get(8)?,
        media_files_json: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

const PRODUCT_COLS: &str = "id, name, description, sku, image_url, tiktok_shop_id, tiktok_url, \
    price, category, media_files_json, created_at, updated_at";

#[tauri::command]
pub fn list_products(state: State<'_, AppState>) -> Result<Vec<Product>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {PRODUCT_COLS} FROM products ORDER BY created_at DESC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], map_product_row)
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CreateProductInput {
    pub name: String,
    pub description: Option<String>,
    pub sku: Option<String>,
    pub image_url: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub tiktok_url: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub media_files_json: Option<String>,
}

#[tauri::command]
pub fn create_product(
    state: State<'_, AppState>,
    input: CreateProductInput,
) -> Result<i64, String> {
    if input.name.trim().is_empty() {
        return Err("Product name is required".to_string());
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        &format!(
            "INSERT INTO products (name, description, sku, image_url, tiktok_shop_id, tiktok_url, price, category, media_files_json, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, {}, {})",
            SQL_NOW_HCM, SQL_NOW_HCM
        ),
        params![
            input.name.trim(),
            input.description,
            input.sku,
            input.image_url,
            input.tiktok_shop_id,
            input.tiktok_url,
            input.price,
            input.category,
            input.media_files_json,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct UpdateProductInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub sku: Option<String>,
    pub image_url: Option<String>,
    pub tiktok_shop_id: Option<String>,
    pub tiktok_url: Option<String>,
    pub price: Option<f64>,
    pub category: Option<String>,
    pub media_files_json: Option<String>,
}

#[tauri::command]
pub fn update_product(
    state: State<'_, AppState>,
    product_id: i64,
    input: UpdateProductInput,
) -> Result<(), String> {
    let mut sets: Vec<String> = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx: usize = 1;

    if let Some(ref name) = input.name {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err("Product name cannot be empty".to_string());
        }
        sets.push(format!("name = ?{idx}"));
        params_vec.push(Box::new(trimmed.to_string()));
        idx += 1;
    }
    if let Some(ref v) = input.description {
        sets.push(format!("description = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = input.sku {
        sets.push(format!("sku = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = input.image_url {
        sets.push(format!("image_url = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = input.tiktok_shop_id {
        sets.push(format!("tiktok_shop_id = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = input.tiktok_url {
        sets.push(format!("tiktok_url = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(price) = input.price {
        sets.push(format!("price = ?{idx}"));
        params_vec.push(Box::new(price));
        idx += 1;
    }
    if let Some(ref v) = input.category {
        sets.push(format!("category = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }
    if let Some(ref v) = input.media_files_json {
        sets.push(format!("media_files_json = ?{idx}"));
        params_vec.push(Box::new(v.clone()));
        idx += 1;
    }

    if sets.is_empty() {
        return Ok(());
    }

    sets.push(format!("updated_at = {SQL_NOW_HCM}"));
    let sql = format!(
        "UPDATE products SET {} WHERE id = ?{}",
        sets.join(", "),
        idx
    );
    params_vec.push(Box::new(product_id));

    let refs: Vec<&dyn rusqlite::types::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let n = conn
        .execute(&sql, refs.as_slice())
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("Product {product_id} not found"));
    }
    Ok(())
}

#[tauri::command]
pub fn delete_product(state: State<'_, AppState>, product_id: i64) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM clip_products WHERE product_id = ?1",
        [product_id],
    )
    .map_err(|e| format!("Could not unlink clips from product {product_id}: {e}"))?;
    let n = conn
        .execute("DELETE FROM products WHERE id = ?1", [product_id])
        .map_err(|e| format!("Could not delete product {product_id}: {e}"))?;
    if n == 0 {
        return Err(format!(
            "Product {product_id} was not found (already deleted?)"
        ));
    }
    Ok(())
}

#[tauri::command]
pub fn tag_clip_product(
    state: State<'_, AppState>,
    clip_id: i64,
    product_id: i64,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR IGNORE INTO clip_products (clip_id, product_id) VALUES (?1, ?2)",
        params![clip_id, product_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
