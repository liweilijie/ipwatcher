//! Library entry that exposes internal modules.
//! Keep public API minimal and explicit.

pub mod config;
pub mod ip;
pub mod db;

// Re-export most used items for ergonomic main.rs imports.
pub use config::{load_from, Config, SmtpConfig};
pub use ip::query_external_ip;
pub use db::{init_db, get_last_ip, save_ip};
