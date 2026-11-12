use serde::{Deserialize, Serialize};

/// Kind of database object the app can script.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectKind {
    Procedure,
    Function,
    View,
    Table,
}

impl ObjectKind {
    /// `sys.objects.type` codes belonging to this kind.
    pub fn type_codes(self) -> &'static [&'static str] {
        match self {
            ObjectKind::Procedure => &["P"],
            ObjectKind::Function => &["FN", "IF", "TF"],
            ObjectKind::View => &["V"],
            ObjectKind::Table => &["U"],
        }
    }

    pub fn from_type_code(code: &str) -> Option<Self> {
        match code.trim() {
            "P" => Some(ObjectKind::Procedure),
            "FN" | "IF" | "TF" => Some(ObjectKind::Function),
            "V" => Some(ObjectKind::View),
            "U" => Some(ObjectKind::Table),
            _ => None,
        }
    }

    pub const ALL: [ObjectKind; 4] = [ObjectKind::Procedure, ObjectKind::Function, ObjectKind::View, ObjectKind::Table];
}

/// Filter sent from the UI when searching for changed objects.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilter {
    /// Empty = all kinds.
    #[serde(default)]
    pub kinds: Vec<ObjectKind>,
    /// Inclusive lower bound for `modify_date`, ISO 8601 (`2026-11-11T00:00:00`). Server local time.
    pub modified_from: Option<String>,
    /// Exclusive upper bound for `modify_date`, ISO 8601.
    pub modified_to: Option<String>,
    /// Substring match on object name.
    pub name_contains: Option<String>,
}

/// One row of the search result grid.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DbObject {
    pub object_id: i32,
    pub schema_name: String,
    pub object_name: String,
    pub kind: ObjectKind,
    pub type_code: String,
    pub type_desc: String,
    pub create_date: String,
    pub modify_date: String,
    /// `modify_date > create_date`, i.e. altered after creation.
    pub is_modified: bool,
}

/// Identifies an object to script. Resolved by name at export time, so it survives DROP/CREATE.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRef {
    pub schema_name: String,
    pub object_name: String,
    pub kind: ObjectKind,
}

impl ObjectRef {
    pub fn display_name(&self) -> String {
        format!("{}.{}", self.schema_name, self.object_name)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub server_name: String,
    pub database: String,
    pub product_version: String,
    pub major_version: i32,
}

// ---- Table metadata (input for the table script generator) -----------------------------

#[derive(Debug, Clone, Default)]
pub struct TableMeta {
    pub columns: Vec<ColumnMeta>,
    pub primary_key: Option<KeyMeta>,
    pub indexes: Vec<IndexMeta>,
    pub foreign_keys: Vec<ForeignKeyMeta>,
    pub checks: Vec<CheckMeta>,
}

#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    /// Full column definition as it appears inside CREATE TABLE / ALTER TABLE ADD,
    /// e.g. `[Id] int IDENTITY(1,1) NOT NULL`.
    pub definition: String,
    pub is_nullable: bool,
    pub has_default: bool,
    pub is_computed: bool,
}

#[derive(Debug, Clone)]
pub struct KeyMeta {
    pub name: String,
    /// `CLUSTERED` / `NONCLUSTERED`
    pub type_desc: String,
    /// Already quoted, e.g. `[Id] ASC, [Code] DESC`
    pub columns: String,
}

#[derive(Debug, Clone)]
pub struct IndexMeta {
    pub name: String,
    pub type_desc: String,
    pub is_unique: bool,
    pub is_unique_constraint: bool,
    pub key_columns: String,
    pub included_columns: Option<String>,
    pub filter: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ForeignKeyMeta {
    pub name: String,
    pub columns: String,
    /// Quoted `[schema].[table]`
    pub referenced_table: String,
    pub referenced_columns: String,
    /// `NO_ACTION`, `CASCADE`, `SET_NULL`, `SET_DEFAULT`
    pub on_delete: String,
    pub on_update: String,
}

#[derive(Debug, Clone)]
pub struct CheckMeta {
    pub name: String,
    pub definition: String,
}

/// Programmable module (procedure / function / view) definition.
#[derive(Debug, Clone)]
pub struct ModuleMeta {
    pub definition: String,
    pub uses_ansi_nulls: bool,
    pub uses_quoted_identifier: bool,
}
