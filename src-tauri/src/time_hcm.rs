//! SQLite TEXT timestamps as wall clock in **Asia/Ho_Chi_Minh** (UTC+7, no DST).

use chrono::{FixedOffset, Utc};

/// Use in raw SQL for `UPDATE` / `INSERT` fragments (matches SQLite UTC `now` + 7h).
pub const SQL_NOW_HCM: &str = "datetime('now', '+7 hours')";

/// Bound parameter `YYYY-MM-DD HH:MM:SS` for the same instant as [`SQL_NOW_HCM`].
pub fn now_timestamp_hcm() -> String {
    let offset = FixedOffset::east_opt(7 * 3600).expect("GMT+7 offset");
    Utc::now()
        .with_timezone(&offset)
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}
