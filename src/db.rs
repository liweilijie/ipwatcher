use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::{net::IpAddr, path::Path};
use time::OffsetDateTime;

/// Initialize the SQLite database (create file and tables if needed).
pub fn init_db(db_path: &str) -> Result<Connection> {
    if let Some(parent) = Path::new(db_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).ok();
        }
    }
    let conn = Connection::open(db_path)
        .with_context(|| format!("Cannot open/create database: {db_path}"))?;
    conn.execute_batch(
        r#"
        PRAGMA journal_mode=WAL;
        CREATE TABLE IF NOT EXISTS ip_history (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            ip          TEXT NOT NULL,
            changed_at  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ip_history_changed_at ON ip_history(changed_at);
        "#,
    )?;
    Ok(conn)
}

/// Read the latest recorded IP from DB (if any).
pub fn get_last_ip(conn: &Connection) -> Result<Option<IpAddr>> {
    let mut stmt = conn.prepare("SELECT ip FROM ip_history ORDER BY id DESC LIMIT 1")?;
    let mut rows = stmt.query([])?;
    if let Some(row) = rows.next()? {
        let ip_str: String = row.get(0)?;
        let ip = ip_str
            .parse::<IpAddr>()
            .map_err(|_| anyhow::anyhow!("Failed to parse IP from DB"))?;
        Ok(Some(ip))
    } else {
        Ok(None)
    }
}

/// Save a new IP entry with timestamp.
pub fn save_ip(conn: &Connection, ip: IpAddr) -> Result<()> {
    let now = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown-time".into());
    conn.execute(
        "INSERT INTO ip_history (ip, changed_at) VALUES (?1, ?2)",
        params![ip.to_string(), now],
    )?;
    Ok(())
}
