//! Local SQLite (`.db3`) store: saved SQL Server connections and app settings.
//! Passwords are stored only encrypted (see [`crate::crypto`]).

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use sqlx::{FromRow, Row};

use crate::connection::{AuthKind, ConnectionParams};
use std::sync::Arc;

use crate::crypto::LazyCipher;
use crate::error::{Error, Result};

/// Current schema version, stored in `PRAGMA user_version`.
/// v1: connections(connection_string) – plaintext, legacy
/// v2: structured connection fields + encrypted password, settings table
const SCHEMA_VERSION: i64 = 2;

/// Connection as sent to the UI. Never contains the password.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: i64,
    pub name: String,
    #[serde(flatten)]
    pub params: ConnectionParams,
    pub has_password: bool,
    /// Derived, read-only, password masked.
    pub connection_string: String,
    pub last_export_dir: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInput {
    pub name: String,
    #[serde(flatten)]
    pub params: ConnectionParams,
    /// `None` when editing = keep the stored password. `Some("")` = clear it.
    pub password: Option<String>,
}

#[derive(FromRow)]
struct ConnectionRow {
    id: i64,
    name: String,
    server: String,
    port: Option<i64>,
    database: Option<String>,
    auth: String,
    username: Option<String>,
    password_enc: Option<String>,
    encrypt: bool,
    trust_server_certificate: bool,
    last_export_dir: Option<String>,
    created_at: String,
    updated_at: String,
}

impl ConnectionRow {
    fn params(&self) -> ConnectionParams {
        ConnectionParams {
            server: self.server.clone(),
            port: self.port.and_then(|p| u16::try_from(p).ok()),
            database: self.database.clone(),
            auth: AuthKind::parse(&self.auth),
            username: self.username.clone(),
            encrypt: self.encrypt,
            trust_server_certificate: self.trust_server_certificate,
        }
    }

    fn into_connection(self) -> Connection {
        let params = self.params();
        let has_password = self.password_enc.is_some();
        Connection {
            connection_string: params.display_string(has_password),
            id: self.id,
            name: self.name,
            params,
            has_password,
            last_export_dir: self.last_export_dir,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Clone)]
pub struct ConnectionStore {
    pool: SqlitePool,
    cipher: Arc<LazyCipher>,
}

/// Result of opening the store.
pub struct OpenedStore {
    pub store: ConnectionStore,
    /// The `.db3` file did not exist and was created.
    pub created: bool,
    /// Number of legacy plaintext connections converted to the encrypted format.
    pub migrated_connections: usize,
}

const CREATE_SCHEMA: &str = r#"
CREATE TABLE settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE connections (
    id                       INTEGER PRIMARY KEY AUTOINCREMENT,
    name                     TEXT    NOT NULL UNIQUE COLLATE NOCASE,
    server                   TEXT    NOT NULL,
    port                     INTEGER NULL,
    database                 TEXT    NULL,
    auth                     TEXT    NOT NULL DEFAULT 'sql' CHECK (auth IN ('sql', 'windows')),
    username                 TEXT    NULL,
    password_enc             TEXT    NULL,
    encrypt                  INTEGER NOT NULL DEFAULT 1,
    trust_server_certificate INTEGER NOT NULL DEFAULT 0,
    last_export_dir          TEXT    NULL,
    created_at               TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at               TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
"#;

const INSERT_SQL: &str = "INSERT INTO connections
    (name, server, port, database, auth, username, password_enc, encrypt, trust_server_certificate)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)";

impl ConnectionStore {
    /// Opens the store at `path`. A missing file is created (with parent directories) and
    /// the schema is initialised; an old plaintext store is migrated and compacted.
    pub async fn open(path: &Path, cipher: LazyCipher) -> Result<OpenedStore> {
        let created = !path.exists();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            // Overwrite deleted content with zeros, so removed secrets do not linger in free pages.
            .pragma("secure_delete", "on");

        let pool = SqlitePoolOptions::new().max_connections(4).connect_with(options).await?;
        let store = Self { pool, cipher: Arc::new(cipher) };
        let migrated_connections = store.migrate().await?;

        if created {
            tracing::info!(path = %path.display(), "created new connection store");
        }
        Ok(OpenedStore { store, created, migrated_connections })
    }

    async fn migrate(&self) -> Result<usize> {
        let (version,): (i64,) = sqlx::query_as("PRAGMA user_version").fetch_one(&self.pool).await?;
        let mut migrated = 0;

        if version == 0 {
            sqlx::raw_sql(CREATE_SCHEMA).execute(&self.pool).await?;
        } else if version == 1 {
            migrated = self.migrate_v1_to_v2().await?;
        }

        if version != SCHEMA_VERSION {
            sqlx::query(&format!("PRAGMA user_version = {SCHEMA_VERSION}")).execute(&self.pool).await?;
        }
        if migrated > 0 {
            // Purge the old plaintext rows from the WAL and free pages.
            sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(&self.pool).await?;
            sqlx::query("VACUUM").execute(&self.pool).await?;
        }
        Ok(migrated)
    }

    /// v1 kept full connection strings (including passwords) in plain text.
    async fn migrate_v1_to_v2(&self) -> Result<usize> {
        let mut tx = self.pool.begin().await?;
        let old = sqlx::query("SELECT id, name, connection_string, last_export_dir, created_at, updated_at FROM connections")
            .fetch_all(&mut *tx)
            .await?;

        sqlx::query("ALTER TABLE connections RENAME TO connections_v1").execute(&mut *tx).await?;
        sqlx::raw_sql(CREATE_SCHEMA).execute(&mut *tx).await?;

        for row in &old {
            let cs: String = row.get("connection_string");
            let (params, password) = ConnectionParams::from_ado_string(&cs);
            let password_enc = password.map(|p| self.cipher.get()?.encrypt(&p)).transpose()?;
            sqlx::query(
                "INSERT INTO connections
                 (id, name, server, port, database, auth, username, password_enc, encrypt, trust_server_certificate,
                  last_export_dir, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(row.get::<i64, _>("id"))
            .bind(row.get::<String, _>("name"))
            .bind(&params.server)
            .bind(params.port.map(i64::from))
            .bind(&params.database)
            .bind(params.auth.as_str())
            .bind(&params.username)
            .bind(password_enc)
            .bind(params.encrypt)
            .bind(params.trust_server_certificate)
            .bind(row.get::<Option<String>, _>("last_export_dir"))
            .bind(row.get::<String, _>("created_at"))
            .bind(row.get::<String, _>("updated_at"))
            .execute(&mut *tx)
            .await?;
        }

        sqlx::query("DROP TABLE connections_v1").execute(&mut *tx).await?;
        tx.commit().await?;
        tracing::info!(count = old.len(), "migrated connection store v1 -> v2 (passwords encrypted)");
        Ok(old.len())
    }

    // ---- connections ------------------------------------------------------------------

    async fn row(&self, id: i64) -> Result<ConnectionRow> {
        sqlx::query_as::<_, ConnectionRow>("SELECT * FROM connections WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| Error::NotFound(format!("#{id}")))
    }

    pub async fn list(&self) -> Result<Vec<Connection>> {
        Ok(sqlx::query_as::<_, ConnectionRow>("SELECT * FROM connections ORDER BY name COLLATE NOCASE")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(ConnectionRow::into_connection)
            .collect())
    }

    pub async fn get(&self, id: i64) -> Result<Connection> {
        Ok(self.row(id).await?.into_connection())
    }

    /// Params plus decrypted password, for connecting. Never sent to the UI.
    /// Returns `Error::DecryptionFailed` when the stored password cannot be decrypted
    /// (e.g. the encryption key changed). The caller can recover by asking for the password.
    pub async fn credentials(&self, id: i64) -> Result<(ConnectionParams, Option<String>)> {
        let row = self.row(id).await?;
        let Some(encrypted) = row.password_enc.as_deref() else {
            return Ok((row.params(), None));
        };
        let cipher = self.cipher.get()?;
        let password = Some(encrypted)
            .map(|p| cipher.decrypt(p))
            .transpose()
            .map_err(|_| Error::DecryptionFailed(id))?;
        Ok((row.params(), password))
    }

    /// Returns only the connection params (no password decryption). Used when password
    /// will be supplied externally (e.g. after a decryption failure).
    pub async fn credentials_params_only(&self, id: i64) -> Result<(ConnectionParams, bool)> {
        let row = self.row(id).await?;
        let has_password = row.password_enc.is_some();
        Ok((row.params(), has_password))
    }

    /// Re-encrypts and saves a password with the current key. Used when the old key is lost.
    pub async fn update_password(&self, id: i64, password: &str) -> Result<()> {
        let password_enc = self.encrypt_password(Some(password))?;
        sqlx::query("UPDATE connections SET password_enc = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?")
            .bind(password_enc)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    fn validate(input: &ConnectionInput) -> Result<String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(Error::validation("Nazwa połączenia jest wymagana", "Connection name is required"));
        }
        input.params.validate()?;
        Ok(name.to_owned())
    }

    fn encrypt_password(&self, password: Option<&str>) -> Result<Option<String>> {
        password.filter(|p| !p.is_empty()).map(|p| self.cipher.get()?.encrypt(p)).transpose()
    }

    pub async fn create(&self, input: &ConnectionInput) -> Result<Connection> {
        let name = Self::validate(input)?;
        let p = &input.params;
        let password_enc = if p.auth == AuthKind::Sql { self.encrypt_password(input.password.as_deref())? } else { None };
        let id = sqlx::query(INSERT_SQL)
            .bind(&name)
            .bind(p.server.trim())
            .bind(p.port.map(i64::from))
            .bind(trimmed(&p.database))
            .bind(p.auth.as_str())
            .bind(trimmed(&p.username))
            .bind(password_enc)
            .bind(p.encrypt)
            .bind(p.trust_server_certificate)
            .execute(&self.pool)
            .await
            .map_err(map_unique)?
            .last_insert_rowid();
        self.get(id).await
    }

    pub async fn update(&self, id: i64, input: &ConnectionInput) -> Result<Connection> {
        let name = Self::validate(input)?;
        let p = &input.params;
        let existing = self.row(id).await?;

        let password_enc = match (p.auth, input.password.as_deref()) {
            (AuthKind::Windows, _) => None,
            (AuthKind::Sql, None) => existing.password_enc,
            (AuthKind::Sql, Some(new)) => self.encrypt_password(Some(new))?,
        };

        sqlx::query(
            "UPDATE connections
             SET name = ?, server = ?, port = ?, database = ?, auth = ?, username = ?, password_enc = ?,
                 encrypt = ?, trust_server_certificate = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?",
        )
        .bind(&name)
        .bind(p.server.trim())
        .bind(p.port.map(i64::from))
        .bind(trimmed(&p.database))
        .bind(p.auth.as_str())
        .bind(trimmed(&p.username))
        .bind(password_enc)
        .bind(p.encrypt)
        .bind(p.trust_server_certificate)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_unique)?;
        self.get(id).await
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM connections WHERE id = ?").bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_last_export_dir(&self, id: i64, dir: &str) -> Result<()> {
        sqlx::query("UPDATE connections SET last_export_dir = ? WHERE id = ?").bind(dir).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    // ---- settings ---------------------------------------------------------------------

    pub async fn settings(&self) -> Result<HashMap<String, String>> {
        Ok(sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings").fetch_all(&self.pool).await?.into_iter().collect())
    }

    pub async fn setting(&self, key: &str) -> Result<Option<String>> {
        Ok(sqlx::query_scalar("SELECT value FROM settings WHERE key = ?").bind(key).fetch_optional(&self.pool).await?)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(key)
            .bind(value)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn trimmed(v: &Option<String>) -> Option<&str> {
    v.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn map_unique(e: sqlx::Error) -> Error {
    match e.as_database_error() {
        Some(db) if db.is_unique_violation() => Error::validation("Połączenie o tej nazwie już istnieje", "A connection with this name already exists"),
        _ => Error::Store(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::crypto::Cipher;

    fn cipher() -> LazyCipher {
        LazyCipher::ready(Cipher::from_key_b64(&Cipher::generate_key_b64()).unwrap())
    }

    fn input(name: &str, password: Option<&str>) -> ConnectionInput {
        ConnectionInput {
            name: name.into(),
            params: ConnectionParams { server: "localhost".into(), database: Some("X".into()), username: Some("sa".into()), encrypt: true, ..Default::default() },
            password: password.map(Into::into),
        }
    }

    async fn file_contains(path: &Path, needle: &str) -> bool {
        let mut bytes = std::fs::read(path).unwrap();
        let wal = path.with_extension("db3-wal");
        if wal.exists() {
            bytes.extend(std::fs::read(wal).unwrap());
        }
        bytes.windows(needle.len()).any(|w| w == needle.as_bytes())
    }

    #[tokio::test]
    async fn creates_file_crud_and_never_stores_plaintext() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("connections.db3");
        let key = Cipher::generate_key_b64();
        let c = || LazyCipher::ready(Cipher::from_key_b64(&key).unwrap());

        let opened = ConnectionStore::open(&path, c()).await.unwrap();
        assert!(opened.created);
        let store = opened.store;

        let a = store.create(&input("Prod", Some("Sup3r$ecret!"))).await.unwrap();
        assert!(a.has_password);
        assert!(a.connection_string.contains("Password=********"));
        store.create(&input("dev", None)).await.unwrap();
        assert!(matches!(store.create(&input("prod", None)).await, Err(Error::Validation(_))));

        let (_, pw) = store.credentials(a.id).await.unwrap();
        assert_eq!(pw.as_deref(), Some("Sup3r$ecret!"));
        assert!(!file_contains(&path, "Sup3r$ecret!").await);

        // Editing without a password keeps the stored one.
        let mut edit = input("Prod 2", None);
        edit.params.port = Some(1444);
        let updated = store.update(a.id, &edit).await.unwrap();
        assert!(updated.has_password);
        assert_eq!(updated.params.port, Some(1444));
        assert_eq!(store.credentials(a.id).await.unwrap().1.as_deref(), Some("Sup3r$ecret!"));

        // Empty string clears it.
        assert!(!store.update(a.id, &input("Prod 2", Some(""))).await.unwrap().has_password);

        let names: Vec<_> = store.list().await.unwrap().into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["dev", "Prod 2"]);

        store.delete(a.id).await.unwrap();
        assert!(matches!(store.get(a.id).await, Err(Error::NotFound(_))));
        drop(store);

        let reopened = ConnectionStore::open(&path, c()).await.unwrap();
        assert!(!reopened.created);
        assert_eq!(reopened.store.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn settings_upsert() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConnectionStore::open(&dir.path().join("c.db3"), cipher()).await.unwrap().store;
        assert_eq!(store.setting("language").await.unwrap(), None);
        store.set_setting("language", "pl").await.unwrap();
        store.set_setting("language", "en").await.unwrap();
        assert_eq!(store.setting("language").await.unwrap().as_deref(), Some("en"));
        assert_eq!(store.settings().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn migrates_v1_plaintext_store() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("connections.db3");

        // Build a v1 database exactly as version 0.1.0 created it.
        {
            let opts = SqliteConnectOptions::new().filename(&path).create_if_missing(true).journal_mode(SqliteJournalMode::Wal);
            let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
            sqlx::raw_sql(
                "CREATE TABLE connections (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL UNIQUE COLLATE NOCASE,
                    connection_string TEXT NOT NULL, last_export_dir TEXT NULL,
                    created_at TEXT NOT NULL DEFAULT 'x', updated_at TEXT NOT NULL DEFAULT 'x');
                 INSERT INTO connections (name, connection_string, last_export_dir)
                    VALUES ('Prod', 'Server=tcp:sqlprod,1433;Database=Sales;User Id=app;Password=PlainP@ss;TrustServerCertificate=true', '/out');
                 PRAGMA user_version = 1;",
            )
            .execute(&pool)
            .await
            .unwrap();
            pool.close().await;
        }
        assert!(file_contains(&path, "PlainP@ss").await);

        let opened = ConnectionStore::open(&path, cipher()).await.unwrap();
        assert_eq!(opened.migrated_connections, 1);
        let store = opened.store;

        let c = &store.list().await.unwrap()[0];
        assert_eq!(c.params.server, "sqlprod");
        assert_eq!(c.params.port, Some(1433));
        assert_eq!(c.params.database.as_deref(), Some("Sales"));
        assert!(c.params.trust_server_certificate);
        assert_eq!(c.last_export_dir.as_deref(), Some("/out"));
        assert_eq!(store.credentials(c.id).await.unwrap().1.as_deref(), Some("PlainP@ss"));
        store.pool.close().await;
        assert!(!file_contains(&path, "PlainP@ss").await, "plaintext must be purged after migration");
    }
}
