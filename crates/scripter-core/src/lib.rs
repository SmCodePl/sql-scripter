//! Core logic of SQL Scripter, independent of Tauri so it can be unit-tested and reused.

pub mod connection;
pub mod crypto;
pub mod error;
pub mod export;
pub mod i18n;
pub mod model;
pub mod mssql;
pub mod scripting;
pub mod store;

pub use error::{Error, Result};
