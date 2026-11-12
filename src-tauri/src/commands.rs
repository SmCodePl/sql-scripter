use std::path::PathBuf;

use scripter_core::connection::ConnectionParams;
use scripter_core::export::{self, ExportReport};
use scripter_core::i18n::{self, Lang};
use scripter_core::model::{DbObject, ObjectRef, SearchFilter, ServerInfo};
use scripter_core::mssql;
use scripter_core::store::{Connection, ConnectionInput};
use serde::{Serialize, Serializer};
use tauri::State;

use crate::state::AppState;

/// Error returned to the frontend as a plain string (caught as `err` in `invoke().catch`).
#[derive(Debug)]
pub struct CommandError(String);

impl From<scripter_core::Error> for CommandError {
    fn from(e: scripter_core::Error) -> Self {
        Self(e.to_string())
    }
}

impl Serialize for CommandError {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.0)
    }
}

type CmdResult<T> = Result<T, CommandError>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    db_path: String,
    db_created: bool,
    migrated_connections: usize,
    version: &'static str,
    /// Saved UI language; `None` = not chosen yet (UI falls back to the system language).
    language: Option<Lang>,
}

#[tauri::command]
pub async fn app_info(state: State<'_, AppState>) -> CmdResult<AppInfo> {
    let language = state.store.setting(SETTING_LANGUAGE).await?.as_deref().and_then(Lang::from_code);
    Ok(AppInfo {
        db_path: state.db_path.display().to_string(),
        db_created: state.db_created,
        migrated_connections: state.migrated_connections,
        version: env!("CARGO_PKG_VERSION"),
        language,
    })
}

// ---- Settings -------------------------------------------------------------------------

pub const SETTING_LANGUAGE: &str = "language";

#[tauri::command]
pub async fn set_language(state: State<'_, AppState>, language: Lang) -> CmdResult<()> {
    state.store.set_setting(SETTING_LANGUAGE, language.code()).await?;
    i18n::set_lang(language);
    Ok(())
}

// ---- Saved connections ----------------------------------------------------------------

#[tauri::command]
pub async fn list_connections(state: State<'_, AppState>) -> CmdResult<Vec<Connection>> {
    Ok(state.store.list().await?)
}

#[tauri::command]
pub async fn create_connection(state: State<'_, AppState>, input: ConnectionInput) -> CmdResult<Connection> {
    Ok(state.store.create(&input).await?)
}

#[tauri::command]
pub async fn update_connection(state: State<'_, AppState>, id: i64, input: ConnectionInput) -> CmdResult<Connection> {
    let updated = state.store.update(id, &input).await?;
    // Drop the open session so the next "connect" uses the new connection string.
    state.close_session(id).await;
    Ok(updated)
}

#[tauri::command]
pub async fn delete_connection(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    state.close_session(id).await;
    Ok(state.store.delete(id).await?)
}

/// Read-only connection string for the form preview (password masked).
#[tauri::command]
pub fn preview_connection_string(params: ConnectionParams, has_password: bool) -> String {
    params.display_string(has_password)
}

/// Tests the form values. When editing and the password field is left empty,
/// the stored (encrypted) password of `connection_id` is used.
#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    params: ConnectionParams,
    password: Option<String>,
    connection_id: Option<i64>,
) -> CmdResult<ServerInfo> {
    let password = match (password.filter(|p| !p.is_empty()), connection_id) {
        (Some(p), _) => Some(p),
        (None, Some(id)) => state.store.credentials(id).await?.1,
        (None, None) => None,
    };
    let mut client = mssql::connect(&params, password.as_deref()).await?;
    Ok(mssql::server_info(&mut client).await?)
}

// ---- Sessions -------------------------------------------------------------------------

#[tauri::command]
pub async fn open_session(state: State<'_, AppState>, connection_id: i64) -> CmdResult<ServerInfo> {
    Ok(state.open_session(connection_id).await?.info.clone())
}

#[tauri::command]
pub async fn close_session(state: State<'_, AppState>, connection_id: i64) -> CmdResult<()> {
    state.close_session(connection_id).await;
    Ok(())
}

// ---- Search / script / export ---------------------------------------------------------

#[tauri::command]
pub async fn search_objects(state: State<'_, AppState>, connection_id: i64, filter: SearchFilter) -> CmdResult<Vec<DbObject>> {
    let session = state.session(connection_id).await?;
    let mut client = session.client.lock().await;
    match mssql::search_objects(&mut client, &filter).await {
        Err(e) if e.is_connection_lost() => {
            session.reconnect(&state.store, &mut client).await?;
            Ok(mssql::search_objects(&mut client, &filter).await?)
        }
        other => Ok(other?),
    }
}

#[tauri::command]
pub async fn preview_script(state: State<'_, AppState>, connection_id: i64, object: ObjectRef) -> CmdResult<String> {
    let session = state.session(connection_id).await?;
    let header = export::header_for(&session.source_label(), session.info.major_version);
    let mut client = session.client.lock().await;
    match export::script_object(&mut client, &object, &header).await {
        Err(e) if e.is_connection_lost() => {
            session.reconnect(&state.store, &mut client).await?;
            Ok(export::script_object(&mut client, &object, &header).await?)
        }
        other => Ok(other?),
    }
}

#[tauri::command]
pub async fn export_objects(
    state: State<'_, AppState>,
    connection_id: i64,
    objects: Vec<ObjectRef>,
    directory: String,
) -> CmdResult<ExportReport> {
    let session = state.session(connection_id).await?;
    let dir = PathBuf::from(&directory);
    let source = session.source_label();

    let major_version = session.info.major_version;
    let report = {
        let mut client = session.client.lock().await;
        match export::export_objects(&mut client, &objects, &dir, &source, major_version).await {
            Err(e) if e.is_connection_lost() => {
                session.reconnect(&state.store, &mut client).await?;
                export::export_objects(&mut client, &objects, &dir, &source, major_version).await?
            }
            other => other?,
        }
    };

    // Remember the folder for next time; not fatal if it fails.
    if let Err(e) = state.store.set_last_export_dir(connection_id, &directory).await {
        tracing::warn!(error = %e, "could not save last export dir");
    }
    Ok(report)
}
