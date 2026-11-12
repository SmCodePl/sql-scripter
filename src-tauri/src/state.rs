use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use scripter_core::i18n::t;
use scripter_core::model::ServerInfo;
use scripter_core::mssql::{self, MssqlClient};
use scripter_core::store::ConnectionStore;
use scripter_core::{Error, Result};
use tokio::sync::Mutex;

/// An open SQL Server connection. Each saved connection can have at most one session,
/// but any number of sessions (databases) can be open at the same time.
pub struct Session {
    pub connection_id: i64,
    pub info: ServerInfo,
    pub client: Mutex<MssqlClient>,
}

impl Session {
    pub fn source_label(&self) -> String {
        format!("{} / {}", self.info.server_name, self.info.database)
    }

    /// Replaces a dead client with a fresh one. Credentials are re-read (and decrypted)
    /// from the store, so the password is not kept in memory for the session's lifetime.
    pub async fn reconnect(&self, store: &ConnectionStore, client: &mut MssqlClient) -> Result<()> {
        tracing::warn!(source = %self.source_label(), "connection lost, reconnecting");
        let (params, password) = store.credentials(self.connection_id).await?;
        *client = mssql::connect(&params, password.as_deref()).await?;
        Ok(())
    }
}

pub struct AppState {
    pub store: ConnectionStore,
    pub db_path: PathBuf,
    pub db_created: bool,
    pub migrated_connections: usize,
    sessions: Mutex<HashMap<i64, Arc<Session>>>,
}

impl AppState {
    pub fn new(store: ConnectionStore, db_path: PathBuf, db_created: bool, migrated_connections: usize) -> Self {
        Self { store, db_path, db_created, migrated_connections, sessions: Mutex::new(HashMap::new()) }
    }

    /// Returns the existing session or opens a new one.
    pub async fn open_session(&self, connection_id: i64) -> Result<Arc<Session>> {
        if let Some(existing) = self.sessions.lock().await.get(&connection_id) {
            return Ok(existing.clone());
        }

        // Connect without holding the map lock, so other tabs stay responsive.
        let (params, password) = self.store.credentials(connection_id).await?;
        let mut client = mssql::connect(&params, password.as_deref()).await?;
        let info = mssql::server_info(&mut client).await?;
        let session = Arc::new(Session { connection_id, info, client: Mutex::new(client) });

        let mut sessions = self.sessions.lock().await;
        // Another call may have won the race; keep the first one.
        Ok(sessions.entry(connection_id).or_insert(session).clone())
    }

    /// Opens a session using an explicit password (when the stored one can't be decrypted).
    pub async fn open_session_with_password(&self, connection_id: i64, password: &str) -> Result<ServerInfo> {
        // Drop any stale session.
        self.sessions.lock().await.remove(&connection_id);

        let (params, _) = self.store.credentials_params_only(connection_id).await?;
        let mut client = mssql::connect(&params, Some(password)).await?;
        let info = mssql::server_info(&mut client).await?;
        let session = Arc::new(Session { connection_id, info: info.clone(), client: Mutex::new(client) });

        let mut sessions = self.sessions.lock().await;
        sessions.insert(connection_id, session);
        Ok(info)
    }

    pub async fn session(&self, connection_id: i64) -> Result<Arc<Session>> {
        self.sessions
            .lock()
            .await
            .get(&connection_id)
            .cloned()
            .ok_or_else(|| Error::Validation(t("Połączenie nie jest otwarte", "Connection is not open").into()))
    }

    pub async fn close_session(&self, connection_id: i64) {
        self.sessions.lock().await.remove(&connection_id);
    }
}
