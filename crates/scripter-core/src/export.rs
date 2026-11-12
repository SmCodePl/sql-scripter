//! Scripting objects and writing `schema.name.sql` files.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{Error, Result};
use crate::model::{ObjectKind, ObjectRef};
use crate::mssql::{self, MssqlClient};
use crate::scripting::{self, ScriptHeader};

/// UTF-8 BOM so SSMS opens files with Polish characters correctly.
const UTF8_BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFailure {
    pub object: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub directory: String,
    pub written: Vec<String>,
    pub failed: Vec<ExportFailure>,
}

pub fn header_for(source: &str) -> ScriptHeader {
    ScriptHeader { source: source.to_owned(), generated_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string() }
}

/// Builds the script text for a single object.
pub async fn script_object(client: &mut MssqlClient, object: &ObjectRef, header: &ScriptHeader) -> Result<String> {
    match object.kind {
        ObjectKind::Table => {
            let meta = mssql::fetch_table(client, object).await?;
            Ok(scripting::table_script(object, &meta, header))
        }
        ObjectKind::Procedure | ObjectKind::Function | ObjectKind::View => {
            let module = mssql::fetch_module(client, object).await?;
            Ok(scripting::module_script(object, &module, header))
        }
    }
}

/// Writes one script file, overwriting an existing one. Returns the file path.
pub async fn write_script(directory: &Path, object: &ObjectRef, script: &str) -> Result<PathBuf> {
    let path = directory.join(scripting::file_name(&object.schema_name, &object.object_name));
    let mut bytes = Vec::with_capacity(UTF8_BOM.len() + script.len());
    bytes.extend_from_slice(UTF8_BOM);
    bytes.extend_from_slice(script.as_bytes());
    tokio::fs::write(&path, bytes).await?;
    Ok(path)
}

/// Scripts and writes every object. Per-object errors are collected, not fatal,
/// except connection loss, which is returned so the caller can reconnect and retry.
pub async fn export_objects(
    client: &mut MssqlClient,
    objects: &[ObjectRef],
    directory: &Path,
    source: &str,
) -> Result<ExportReport> {
    if !directory.is_dir() {
        return Err(Error::Validation(format!(
            "{}: {}",
            crate::i18n::t("Folder docelowy nie istnieje", "Target folder does not exist"),
            directory.display()
        )));
    }

    let header = header_for(source);
    let mut report = ExportReport { directory: directory.display().to_string(), written: Vec::new(), failed: Vec::new() };

    for object in objects {
        let result = match script_object(client, object, &header).await {
            Ok(script) => write_script(directory, object, &script).await,
            Err(e) if e.is_connection_lost() => return Err(e),
            Err(e) => Err(e),
        };
        match result {
            Ok(path) => report
                .written
                .push(path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default()),
            Err(e) => report.failed.push(ExportFailure { object: object.display_name(), error: e.to_string() }),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn writes_bom_and_schema_name_file() {
        let dir = tempfile::tempdir().unwrap();
        let obj = ObjectRef { schema_name: "dbo".into(), object_name: "usp_Test".into(), kind: ObjectKind::Procedure };
        let path = write_script(dir.path(), &obj, "SELECT 'ż'").await.unwrap();
        assert_eq!(path.file_name().unwrap(), "dbo.usp_Test.sql");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(UTF8_BOM));
        assert_eq!(std::str::from_utf8(&bytes[3..]).unwrap(), "SELECT 'ż'");
    }
}
