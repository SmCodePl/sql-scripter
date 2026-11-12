//! Pure script generation (no I/O): CREATE OR ALTER for modules and idempotent table scripts.

use crate::model::{ModuleMeta, ObjectRef, TableMeta};

/// Context printed in the script header.
#[derive(Debug, Clone)]
pub struct ScriptHeader {
    /// e.g. `SQLPROD01 / Sales`
    pub source: String,
    /// e.g. `2026-11-11 15:40:00`
    pub generated_at: String,
}

/// `[name]` with `]` escaped.
pub fn quote_ident(name: &str) -> String {
    format!("[{}]", name.replace(']', "]]"))
}

/// `N'...'` literal with `'` escaped.
pub fn n_literal(value: &str) -> String {
    format!("N'{}'", value.replace('\'', "''"))
}

pub fn qualified_name(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_ident(schema), quote_ident(name))
}

/// `schema.name.sql`, with characters that are invalid in Windows/macOS file names replaced.
pub fn file_name(schema: &str, name: &str) -> String {
    let raw = format!("{schema}.{name}");
    let mut cleaned: String = raw
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    // Windows silently strips trailing dots and spaces.
    while cleaned.ends_with(['.', ' ']) {
        cleaned.pop();
    }
    format!("{cleaned}.sql")
}

/// Normalises line endings to CRLF (what SSMS expects).
pub fn normalize_crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").replace('\n', "\r\n")
}

// ---- Modules: CREATE -> CREATE OR ALTER ------------------------------------------------

/// Rewrites the leading `CREATE` keyword of a module definition to `CREATE OR ALTER`.
/// Leading whitespace and comments (`--`, nested `/* */`) are preserved. If the definition
/// already says `CREATE OR ALTER` (or does not start with CREATE) it is returned unchanged.
pub fn to_create_or_alter(definition: &str) -> String {
    let bytes = definition.as_bytes();
    let start = skip_trivia(bytes, 0);
    if !starts_with_keyword(&bytes[start..], b"CREATE") {
        return definition.to_owned();
    }
    let after_create = start + "CREATE".len();
    let next = skip_trivia(bytes, after_create);
    if starts_with_keyword(&bytes[next..], b"OR") {
        return definition.to_owned();
    }
    format!("{}CREATE OR ALTER{}", &definition[..start], &definition[after_create..])
}

/// Skips whitespace and T-SQL comments starting at `i`; returns the first significant byte index.
fn skip_trivia(s: &[u8], mut i: usize) -> usize {
    loop {
        match s.get(i..) {
            Some([c, ..]) if c.is_ascii_whitespace() => i += 1,
            Some([b'-', b'-', ..]) => {
                while i < s.len() && s[i] != b'\n' {
                    i += 1;
                }
            }
            Some([b'/', b'*', ..]) => {
                // T-SQL block comments nest.
                let mut depth = 0usize;
                while i < s.len() {
                    if s[i..].starts_with(b"/*") {
                        depth += 1;
                        i += 2;
                    } else if s[i..].starts_with(b"*/") {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            _ => return i,
        }
    }
}

fn starts_with_keyword(s: &[u8], keyword: &[u8]) -> bool {
    s.len() >= keyword.len()
        && s[..keyword.len()].eq_ignore_ascii_case(keyword)
        && !s.get(keyword.len()).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'@' | b'#' | b'$'))
}

pub fn module_script(object: &ObjectRef, module: &ModuleMeta, header: &ScriptHeader) -> String {
    let mut out = String::new();
    push_header(&mut out, object, header, None);
    out.push_str(&format!("SET ANSI_NULLS {}\nGO\n", on_off(module.uses_ansi_nulls)));
    out.push_str(&format!("SET QUOTED_IDENTIFIER {}\nGO\n\n", on_off(module.uses_quoted_identifier)));
    out.push_str(to_create_or_alter(module.definition.trim_matches(['\r', '\n'])).trim_end());
    out.push_str("\nGO\n");
    normalize_crlf(&out)
}

fn on_off(v: bool) -> &'static str {
    if v { "ON" } else { "OFF" }
}

// ---- Tables: idempotent "create or update" script --------------------------------------

/// Generates a re-runnable table script:
/// 1. `CREATE TABLE` (with PK) when the table does not exist,
/// 2. `ALTER TABLE ... ADD` for every column that is missing,
/// 3. PK / UNIQUE / indexes / CHECK / FOREIGN KEY added when missing (by name).
///
/// It never drops anything and never changes the type of an existing column.
pub fn table_script(object: &ObjectRef, meta: &TableMeta, header: &ScriptHeader) -> String {
    let table = qualified_name(&object.schema_name, &object.object_name);
    let table_lit = n_literal(&table);
    let mut out = String::new();

    push_header(
        &mut out,
        object,
        header,
        Some(
            "Idempotent script: creates the table if it is missing, then adds missing columns,\n\
             -- keys, indexes and constraints. Existing columns are NOT altered and nothing is dropped.",
        ),
    );

    // 1. CREATE TABLE
    out.push_str(&format!("IF OBJECT_ID({table_lit}, N'U') IS NULL\nBEGIN\n    CREATE TABLE {table} (\n"));
    let mut lines: Vec<String> = meta.columns.iter().map(|c| format!("        {}", c.definition)).collect();
    if let Some(pk) = &meta.primary_key {
        lines.push(format!(
            "        CONSTRAINT {} PRIMARY KEY {} ({})",
            quote_ident(&pk.name),
            pk.type_desc,
            pk.columns
        ));
    }
    out.push_str(&lines.join(",\n"));
    out.push_str("\n    );\nEND\nGO\n\n");

    // 2. Missing columns
    if !meta.columns.is_empty() {
        out.push_str("-- Columns\n");
    }
    for col in &meta.columns {
        if !col.is_nullable && !col.has_default && !col.is_computed {
            out.push_str("-- WARNING: NOT NULL column without DEFAULT - ADD fails if the table already has rows.\n");
        }
        out.push_str(&format!(
            "IF COL_LENGTH({table_lit}, {}) IS NULL\n    ALTER TABLE {table} ADD {};\nGO\n",
            n_literal(&col.name),
            col.definition
        ));
    }

    // 3a. Primary key on an existing table
    if let Some(pk) = &meta.primary_key {
        out.push_str(&format!(
            "\n-- Primary key\nIF NOT EXISTS (SELECT 1 FROM sys.key_constraints WHERE parent_object_id = OBJECT_ID({table_lit}) AND type = 'PK')\n    ALTER TABLE {table} ADD CONSTRAINT {} PRIMARY KEY {} ({});\nGO\n",
            quote_ident(&pk.name),
            pk.type_desc,
            pk.columns
        ));
    }

    // 3b. Indexes and unique constraints
    if !meta.indexes.is_empty() {
        out.push_str("\n-- Indexes\n");
    }
    for ix in &meta.indexes {
        let exists = format!(
            "IF NOT EXISTS (SELECT 1 FROM sys.indexes WHERE object_id = OBJECT_ID({table_lit}) AND name = {})\n",
            n_literal(&ix.name)
        );
        out.push_str(&exists);
        if ix.is_unique_constraint {
            out.push_str(&format!(
                "    ALTER TABLE {table} ADD CONSTRAINT {} UNIQUE {} ({});\nGO\n",
                quote_ident(&ix.name),
                ix.type_desc,
                ix.key_columns
            ));
        } else {
            let mut stmt = format!(
                "    CREATE {}{} INDEX {} ON {table} ({})",
                if ix.is_unique { "UNIQUE " } else { "" },
                ix.type_desc,
                quote_ident(&ix.name),
                ix.key_columns
            );
            if let Some(incl) = ix.included_columns.as_deref().filter(|s| !s.is_empty()) {
                stmt.push_str(&format!(" INCLUDE ({incl})"));
            }
            if let Some(filter) = ix.filter.as_deref().filter(|s| !s.is_empty()) {
                stmt.push_str(&format!(" WHERE {filter}"));
            }
            out.push_str(&stmt);
            out.push_str(";\nGO\n");
        }
    }

    // 3c. CHECK constraints
    if !meta.checks.is_empty() {
        out.push_str("\n-- Check constraints\n");
    }
    for ck in &meta.checks {
        out.push_str(&format!(
            "IF NOT EXISTS (SELECT 1 FROM sys.check_constraints WHERE parent_object_id = OBJECT_ID({table_lit}) AND name = {})\n    ALTER TABLE {table} WITH CHECK ADD CONSTRAINT {} CHECK {};\nGO\n",
            n_literal(&ck.name),
            quote_ident(&ck.name),
            ck.definition
        ));
    }

    // 3d. Foreign keys
    if !meta.foreign_keys.is_empty() {
        out.push_str("\n-- Foreign keys\n");
    }
    for fk in &meta.foreign_keys {
        let mut stmt = format!(
            "IF NOT EXISTS (SELECT 1 FROM sys.foreign_keys WHERE parent_object_id = OBJECT_ID({table_lit}) AND name = {})\n    ALTER TABLE {table} WITH CHECK ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
            n_literal(&fk.name),
            quote_ident(&fk.name),
            fk.columns,
            fk.referenced_table,
            fk.referenced_columns
        );
        if fk.on_delete != "NO_ACTION" {
            stmt.push_str(&format!(" ON DELETE {}", fk.on_delete.replace('_', " ")));
        }
        if fk.on_update != "NO_ACTION" {
            stmt.push_str(&format!(" ON UPDATE {}", fk.on_update.replace('_', " ")));
        }
        out.push_str(&stmt);
        out.push_str(";\nGO\n");
    }

    normalize_crlf(&out)
}

fn push_header(out: &mut String, object: &ObjectRef, header: &ScriptHeader, note: Option<&str>) {
    out.push_str(&format!(
        "-- {:?}: {}\n-- Source: {}\n-- Generated by SQL Scripter: {}\n",
        object.kind,
        qualified_name(&object.schema_name, &object.object_name),
        header.source,
        header.generated_at
    ));
    if let Some(note) = note {
        out.push_str(&format!("-- {note}\n"));
    }
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn header() -> ScriptHeader {
        ScriptHeader { source: "srv / db".into(), generated_at: "2026-11-11 15:00:00".into() }
    }

    #[test]
    fn create_or_alter_basic() {
        assert_eq!(to_create_or_alter("CREATE PROCEDURE dbo.x AS SELECT 1"), "CREATE OR ALTER PROCEDURE dbo.x AS SELECT 1");
        assert_eq!(to_create_or_alter("create   function f()"), "CREATE OR ALTER   function f()");
    }

    #[test]
    fn create_or_alter_keeps_leading_comments() {
        let def = "\r\n-- author: SM\r\n/* outer /* nested */ still comment CREATE */\r\nCREATE PROC p AS SELECT 'CREATE'";
        let out = to_create_or_alter(def);
        assert!(out.starts_with("\r\n-- author: SM\r\n/* outer /* nested */ still comment CREATE */\r\nCREATE OR ALTER PROC p"));
        assert!(out.ends_with("SELECT 'CREATE'"));
    }

    #[test]
    fn create_or_alter_is_idempotent() {
        let def = "CREATE OR ALTER VIEW v AS SELECT 1";
        assert_eq!(to_create_or_alter(def), def);
        let def = "CREATE /* c */ or alter VIEW v AS SELECT 1";
        assert_eq!(to_create_or_alter(def), def);
    }

    #[test]
    fn create_or_alter_ignores_non_create_and_identifiers() {
        assert_eq!(to_create_or_alter("ALTER PROC p AS SELECT 1"), "ALTER PROC p AS SELECT 1");
        assert_eq!(to_create_or_alter("CREATED_BY"), "CREATED_BY");
        assert_eq!(to_create_or_alter("łódź CREATE"), "łódź CREATE");
    }

    #[test]
    fn file_names_are_sanitized() {
        assert_eq!(file_name("dbo", "usp_Get"), "dbo.usp_Get.sql");
        assert_eq!(file_name("dbo", "a/b:c*?"), "dbo.a_b_c__.sql");
        assert_eq!(file_name("dbo", "trailing. "), "dbo.trailing.sql");
        assert_eq!(file_name("sprzedaż", "Zamówienia"), "sprzedaż.Zamówienia.sql");
    }

    #[test]
    fn quoting() {
        assert_eq!(quote_ident("we]ird"), "[we]]ird]");
        assert_eq!(n_literal("O'Neil"), "N'O''Neil'");
    }

    #[test]
    fn module_script_has_set_options_and_go() {
        let obj = ObjectRef { schema_name: "dbo".into(), object_name: "p".into(), kind: ObjectKind::Procedure };
        let m = ModuleMeta { definition: "CREATE PROCEDURE dbo.p AS\nSELECT 1\n".into(), uses_ansi_nulls: true, uses_quoted_identifier: false };
        let s = module_script(&obj, &m, &header());
        assert!(s.contains("SET ANSI_NULLS ON\r\nGO\r\nSET QUOTED_IDENTIFIER OFF\r\nGO"));
        assert!(s.contains("CREATE OR ALTER PROCEDURE dbo.p AS\r\nSELECT 1\r\nGO\r\n"));
        assert!(!s.contains("\r\r"));
    }

    #[test]
    fn table_script_is_idempotent_shape() {
        let obj = ObjectRef { schema_name: "dbo".into(), object_name: "Orders".into(), kind: ObjectKind::Table };
        let meta = TableMeta {
            columns: vec![
                ColumnMeta { name: "Id".into(), definition: "[Id] int IDENTITY(1,1) NOT NULL".into(), is_nullable: false, has_default: false, is_computed: false },
                ColumnMeta { name: "Note".into(), definition: "[Note] nvarchar(200) NULL".into(), is_nullable: true, has_default: false, is_computed: false },
            ],
            primary_key: Some(KeyMeta { name: "PK_Orders".into(), type_desc: "CLUSTERED".into(), columns: "[Id] ASC".into() }),
            indexes: vec![IndexMeta {
                name: "IX_Orders_Note".into(),
                type_desc: "NONCLUSTERED".into(),
                is_unique: false,
                is_unique_constraint: false,
                key_columns: "[Note] ASC".into(),
                included_columns: Some("[Id]".into()),
                filter: Some("([Note] IS NOT NULL)".into()),
            }],
            foreign_keys: vec![ForeignKeyMeta {
                name: "FK_Orders_Customer".into(),
                columns: "[CustomerId]".into(),
                referenced_table: "[dbo].[Customer]".into(),
                referenced_columns: "[Id]".into(),
                on_delete: "CASCADE".into(),
                on_update: "NO_ACTION".into(),
            }],
            checks: vec![CheckMeta { name: "CK_Note".into(), definition: "(len([Note])>(0))".into() }],
        };
        let s = table_script(&obj, &meta, &header());
        assert!(s.contains("IF OBJECT_ID(N'[dbo].[Orders]', N'U') IS NULL"));
        assert!(s.contains("CONSTRAINT [PK_Orders] PRIMARY KEY CLUSTERED ([Id] ASC)"));
        assert!(s.contains("IF COL_LENGTH(N'[dbo].[Orders]', N'Note') IS NULL\r\n    ALTER TABLE [dbo].[Orders] ADD [Note] nvarchar(200) NULL;"));
        assert!(s.contains("-- WARNING: NOT NULL column without DEFAULT"));
        assert!(s.contains("CREATE NONCLUSTERED INDEX [IX_Orders_Note] ON [dbo].[Orders] ([Note] ASC) INCLUDE ([Id]) WHERE ([Note] IS NOT NULL);"));
        assert!(s.contains("REFERENCES [dbo].[Customer] ([Id]) ON DELETE CASCADE;"));
        assert!(!s.contains("ON UPDATE"));
        assert!(s.contains("ADD CONSTRAINT [CK_Note] CHECK (len([Note])>(0));"));
    }
}
