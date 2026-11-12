//! SQL Server access: connecting and reading the catalog (sys.* views).
//! Requires SQL Server 2017+ (STRING_AGG) for table scripting; CREATE OR ALTER needs 2016 SP1+.

use std::time::Duration;

use tiberius::{Client, Config, Row, SqlBrowser};
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::connection::ConnectionParams;
use crate::error::{Error, Result};
use crate::i18n::t;
use crate::model::*;

pub type MssqlClient = Client<Compat<TcpStream>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Connects using structured parameters and the (decrypted) password.
/// Named instances (`HOST\SQLEXPRESS`) are resolved through the SQL Browser service.
pub async fn connect(params: &ConnectionParams, password: Option<&str>) -> Result<MssqlClient> {
    let config = params.to_config(password)?;
    tokio::time::timeout(CONNECT_TIMEOUT, connect_with(config))
        .await
        .map_err(|_| Error::Timeout(CONNECT_TIMEOUT))?
}

async fn connect_with(config: Config) -> Result<MssqlClient> {
    let tcp = TcpStream::connect_named(&config).await?;
    tcp.set_nodelay(true)?;

    match Client::connect(config.clone(), tcp.compat_write()).await {
        Ok(client) => Ok(client),
        // Azure SQL gateway may redirect to another host.
        Err(tiberius::error::Error::Routing { host, port }) => {
            let mut config = config;
            config.host(&host);
            config.port(port);
            let tcp = TcpStream::connect(config.get_addr()).await?;
            tcp.set_nodelay(true)?;
            Ok(Client::connect(config, tcp.compat_write()).await?)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn server_info(client: &mut MssqlClient) -> Result<ServerInfo> {
    let row = client
        .simple_query(
            "SELECT CAST(@@SERVERNAME AS nvarchar(256)),
                    CAST(DB_NAME() AS nvarchar(256)),
                    CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(128)),
                    CAST(PARSENAME(CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(128)), 4) AS int)",
        )
        .await?
        .into_row()
        .await?
        .ok_or_else(|| Error::NotFound("SERVERPROPERTY".into()))?;

    Ok(ServerInfo {
        server_name: str_at(&row, 0)?,
        database: str_at(&row, 1)?,
        product_version: str_at(&row, 2)?,
        major_version: row.get::<i32, _>(3).unwrap_or(0),
    })
}

/// Lists objects matching the filter, newest change first.
pub async fn search_objects(client: &mut MssqlClient, filter: &SearchFilter) -> Result<Vec<DbObject>> {
    let kinds: &[ObjectKind] = if filter.kinds.is_empty() { &ObjectKind::ALL } else { &filter.kinds };
    // Type codes come from a fixed whitelist, so inlining them is safe.
    let type_list = kinds
        .iter()
        .flat_map(|k| k.type_codes())
        .map(|c| format!("'{c}'"))
        .collect::<Vec<_>>()
        .join(",");

    let sql = format!(
        "SELECT o.object_id,
                CAST(s.name AS nvarchar(128)),
                CAST(o.name AS nvarchar(128)),
                CAST(RTRIM(o.type) AS nvarchar(2)),
                CAST(o.type_desc AS nvarchar(60)),
                CONVERT(nvarchar(19), o.create_date, 120),
                CONVERT(nvarchar(19), o.modify_date, 120),
                CAST(CASE WHEN o.modify_date > o.create_date THEN 1 ELSE 0 END AS bit)
         FROM sys.objects o
         JOIN sys.schemas s ON s.schema_id = o.schema_id
         WHERE o.is_ms_shipped = 0
           AND o.type IN ({type_list})
           AND (@P1 IS NULL OR o.modify_date >= CONVERT(datetime2, @P1, 126))
           AND (@P2 IS NULL OR o.modify_date <  CONVERT(datetime2, @P2, 126))
           AND (@P3 IS NULL OR o.name LIKE N'%' + @P3 + N'%')
         ORDER BY o.modify_date DESC, s.name, o.name"
    );

    let from = non_empty(filter.modified_from.as_deref());
    let to = non_empty(filter.modified_to.as_deref());
    let name = non_empty(filter.name_contains.as_deref()).map(escape_like);

    let rows = client.query(sql, &[&from, &to, &name.as_deref()]).await?.into_first_result().await?;

    rows.iter()
        .map(|row| {
            let type_code = str_at(row, 3)?;
            Ok(DbObject {
                object_id: row.get::<i32, _>(0).unwrap_or_default(),
                schema_name: str_at(row, 1)?,
                object_name: str_at(row, 2)?,
                kind: ObjectKind::from_type_code(&type_code)
                    .ok_or_else(|| Error::Validation(format!("{}: {type_code}", t("Nieobsługiwany typ obiektu", "Unsupported object type"))))?,
                type_desc: str_at(row, 4)?,
                create_date: str_at(row, 5)?,
                modify_date: str_at(row, 6)?,
                is_modified: row.get::<bool, _>(7).unwrap_or(false),
                type_code,
            })
        })
        .collect()
}

pub async fn fetch_module(client: &mut MssqlClient, object: &ObjectRef) -> Result<ModuleMeta> {
    let full_name = crate::scripting::qualified_name(&object.schema_name, &object.object_name);
    let row = client
        .query(
            "SELECT m.definition,
                    CAST(ISNULL(m.uses_ansi_nulls, 1) AS bit),
                    CAST(ISNULL(m.uses_quoted_identifier, 1) AS bit)
             FROM sys.sql_modules m
             WHERE m.object_id = OBJECT_ID(@P1)",
            &[&full_name.as_str()],
        )
        .await?
        .into_row()
        .await?
        .ok_or_else(|| Error::NotFound(object.display_name()))?;

    let definition = row
        .get::<&str, _>(0)
        .ok_or_else(|| {
            Error::Validation(format!(
                "{}: {}",
                object.display_name(),
                t("definicja jest niedostępna (WITH ENCRYPTION?)", "definition is not available (WITH ENCRYPTION?)")
            ))
        })?
        .to_owned();

    Ok(ModuleMeta {
        definition,
        uses_ansi_nulls: row.get::<bool, _>(1).unwrap_or(true),
        uses_quoted_identifier: row.get::<bool, _>(2).unwrap_or(true),
    })
}

const COLUMNS_SQL: &str = r#"
SELECT
    CAST(c.name AS nvarchar(128)),
    CAST(
        QUOTENAME(c.name) + N' ' +
        CASE
            WHEN cc.object_id IS NOT NULL THEN
                N'AS ' + cc.definition + CASE WHEN cc.is_persisted = 1 THEN N' PERSISTED' ELSE N'' END
            ELSE
                CASE WHEN ty.is_user_defined = 1
                     THEN QUOTENAME(SCHEMA_NAME(ty.schema_id)) + N'.' + QUOTENAME(ty.name)
                     ELSE ty.name END +
                CASE
                    WHEN ty.is_user_defined = 1 THEN N''
                    WHEN ty.name IN (N'varchar', N'char', N'varbinary', N'binary')
                        THEN N'(' + CASE WHEN c.max_length = -1 THEN N'max' ELSE CAST(c.max_length AS nvarchar(10)) END + N')'
                    WHEN ty.name IN (N'nvarchar', N'nchar')
                        THEN N'(' + CASE WHEN c.max_length = -1 THEN N'max' ELSE CAST(c.max_length / 2 AS nvarchar(10)) END + N')'
                    WHEN ty.name IN (N'decimal', N'numeric')
                        THEN N'(' + CAST(c.precision AS nvarchar(10)) + N',' + CAST(c.scale AS nvarchar(10)) + N')'
                    WHEN ty.name IN (N'datetime2', N'time', N'datetimeoffset')
                        THEN N'(' + CAST(c.scale AS nvarchar(10)) + N')'
                    ELSE N''
                END +
                CASE WHEN c.collation_name IS NOT NULL
                          AND c.collation_name <> CAST(DATABASEPROPERTYEX(DB_NAME(), 'Collation') AS sysname)
                     THEN N' COLLATE ' + c.collation_name ELSE N'' END +
                CASE WHEN ic.object_id IS NOT NULL
                     THEN N' IDENTITY(' + CAST(ic.seed_value AS nvarchar(30)) + N',' + CAST(ic.increment_value AS nvarchar(30)) + N')'
                     ELSE N'' END +
                CASE WHEN c.is_nullable = 1 THEN N' NULL' ELSE N' NOT NULL' END +
                CASE WHEN dc.object_id IS NOT NULL
                     THEN N' CONSTRAINT ' + QUOTENAME(dc.name) + N' DEFAULT ' + dc.definition
                     ELSE N'' END
        END AS nvarchar(max)),
    c.is_nullable,
    CAST(CASE WHEN dc.object_id IS NOT NULL THEN 1 ELSE 0 END AS bit),
    c.is_computed
FROM sys.columns c
JOIN sys.types ty                    ON ty.user_type_id = c.user_type_id
LEFT JOIN sys.computed_columns cc    ON cc.object_id = c.object_id AND cc.column_id = c.column_id
LEFT JOIN sys.identity_columns ic    ON ic.object_id = c.object_id AND ic.column_id = c.column_id
LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id
WHERE c.object_id = OBJECT_ID(@P1)
ORDER BY c.column_id
"#;

const PK_SQL: &str = r#"
SELECT CAST(kc.name AS nvarchar(128)),
       CAST(i.type_desc AS nvarchar(60)),
       CAST(STRING_AGG(CAST(QUOTENAME(col.name) + CASE WHEN ixc.is_descending_key = 1 THEN N' DESC' ELSE N' ASC' END AS nvarchar(max)), N', ')
            WITHIN GROUP (ORDER BY ixc.key_ordinal) AS nvarchar(max))
FROM sys.key_constraints kc
JOIN sys.indexes i         ON i.object_id = kc.parent_object_id AND i.index_id = kc.unique_index_id
JOIN sys.index_columns ixc ON ixc.object_id = i.object_id AND ixc.index_id = i.index_id AND ixc.key_ordinal > 0
JOIN sys.columns col       ON col.object_id = ixc.object_id AND col.column_id = ixc.column_id
WHERE kc.type = 'PK' AND kc.parent_object_id = OBJECT_ID(@P1)
GROUP BY kc.name, i.type_desc
"#;

// Both aggregates share one ordering (SQL Server rejects different WITHIN GROUP orders in one scope);
// included columns have key_ordinal = 0 so they sort first, which does not matter for INCLUDE.
const INDEXES_SQL: &str = r#"
SELECT CAST(i.name AS nvarchar(128)),
       CAST(i.type_desc AS nvarchar(60)),
       i.is_unique,
       i.is_unique_constraint,
       CAST(STRING_AGG(CASE WHEN ixc.is_included_column = 0
                            THEN CAST(QUOTENAME(c.name) + CASE WHEN ixc.is_descending_key = 1 THEN N' DESC' ELSE N' ASC' END AS nvarchar(max)) END, N', ')
            WITHIN GROUP (ORDER BY ixc.key_ordinal, ixc.index_column_id) AS nvarchar(max)),
       CAST(STRING_AGG(CASE WHEN ixc.is_included_column = 1 THEN CAST(QUOTENAME(c.name) AS nvarchar(max)) END, N', ')
            WITHIN GROUP (ORDER BY ixc.key_ordinal, ixc.index_column_id) AS nvarchar(max)),
       CAST(i.filter_definition AS nvarchar(max))
FROM sys.indexes i
JOIN sys.index_columns ixc ON ixc.object_id = i.object_id AND ixc.index_id = i.index_id
JOIN sys.columns c         ON c.object_id = ixc.object_id AND c.column_id = ixc.column_id
WHERE i.object_id = OBJECT_ID(@P1)
  AND i.is_primary_key = 0
  AND i.is_hypothetical = 0
  AND i.type IN (1, 2)
GROUP BY i.name, i.type_desc, i.is_unique, i.is_unique_constraint, i.filter_definition
ORDER BY i.name
"#;

const FOREIGN_KEYS_SQL: &str = r#"
SELECT CAST(fk.name AS nvarchar(128)),
       CAST(STRING_AGG(CAST(QUOTENAME(pc.name) AS nvarchar(max)), N', ') WITHIN GROUP (ORDER BY fkc.constraint_column_id) AS nvarchar(max)),
       CAST(QUOTENAME(rs.name) + N'.' + QUOTENAME(rt.name) AS nvarchar(300)),
       CAST(STRING_AGG(CAST(QUOTENAME(rc.name) AS nvarchar(max)), N', ') WITHIN GROUP (ORDER BY fkc.constraint_column_id) AS nvarchar(max)),
       CAST(fk.delete_referential_action_desc AS nvarchar(60)),
       CAST(fk.update_referential_action_desc AS nvarchar(60))
FROM sys.foreign_keys fk
JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id
JOIN sys.columns pc ON pc.object_id = fkc.parent_object_id     AND pc.column_id = fkc.parent_column_id
JOIN sys.columns rc ON rc.object_id = fkc.referenced_object_id AND rc.column_id = fkc.referenced_column_id
JOIN sys.tables rt  ON rt.object_id = fk.referenced_object_id
JOIN sys.schemas rs ON rs.schema_id = rt.schema_id
WHERE fk.parent_object_id = OBJECT_ID(@P1)
GROUP BY fk.name, rs.name, rt.name, fk.delete_referential_action_desc, fk.update_referential_action_desc
ORDER BY fk.name
"#;

const CHECKS_SQL: &str = r#"
SELECT CAST(cc.name AS nvarchar(128)), CAST(cc.definition AS nvarchar(max))
FROM sys.check_constraints cc
WHERE cc.parent_object_id = OBJECT_ID(@P1)
ORDER BY cc.name
"#;

pub async fn fetch_table(client: &mut MssqlClient, object: &ObjectRef) -> Result<TableMeta> {
    let full_name = crate::scripting::qualified_name(&object.schema_name, &object.object_name);
    let p = full_name.as_str();

    let columns = client
        .query(COLUMNS_SQL, &[&p])
        .await?
        .into_first_result()
        .await?
        .iter()
        .map(|r| {
            Ok(ColumnMeta {
                name: str_at(r, 0)?,
                definition: str_at(r, 1)?,
                is_nullable: r.get::<bool, _>(2).unwrap_or(true),
                has_default: r.get::<bool, _>(3).unwrap_or(false),
                is_computed: r.get::<bool, _>(4).unwrap_or(false),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    if columns.is_empty() {
        return Err(Error::NotFound(object.display_name()));
    }

    let primary_key = client
        .query(PK_SQL, &[&p])
        .await?
        .into_row()
        .await?
        .map(|r| -> Result<KeyMeta> { Ok(KeyMeta { name: str_at(&r, 0)?, type_desc: str_at(&r, 1)?, columns: str_at(&r, 2)? }) })
        .transpose()?;

    let indexes = client
        .query(INDEXES_SQL, &[&p])
        .await?
        .into_first_result()
        .await?
        .iter()
        .map(|r| {
            Ok(IndexMeta {
                name: str_at(r, 0)?,
                type_desc: str_at(r, 1)?,
                is_unique: r.get::<bool, _>(2).unwrap_or(false),
                is_unique_constraint: r.get::<bool, _>(3).unwrap_or(false),
                key_columns: str_at(r, 4)?,
                included_columns: opt_str_at(r, 5),
                filter: opt_str_at(r, 6),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let foreign_keys = client
        .query(FOREIGN_KEYS_SQL, &[&p])
        .await?
        .into_first_result()
        .await?
        .iter()
        .map(|r| {
            Ok(ForeignKeyMeta {
                name: str_at(r, 0)?,
                columns: str_at(r, 1)?,
                referenced_table: str_at(r, 2)?,
                referenced_columns: str_at(r, 3)?,
                on_delete: str_at(r, 4)?,
                on_update: str_at(r, 5)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    let checks = client
        .query(CHECKS_SQL, &[&p])
        .await?
        .into_first_result()
        .await?
        .iter()
        .map(|r| Ok(CheckMeta { name: str_at(r, 0)?, definition: str_at(r, 1)? }))
        .collect::<Result<Vec<_>>>()?;

    Ok(TableMeta { columns, primary_key, indexes, foreign_keys, checks })
}

// ---- helpers -------------------------------------------------------------------------

fn str_at(row: &Row, idx: usize) -> Result<String> {
    row.try_get::<&str, _>(idx)?
        .map(str::to_owned)
        .ok_or_else(|| Error::Validation(format!("{} {idx}", t("Nieoczekiwana wartość NULL w kolumnie", "Unexpected NULL in column"))))
}

fn opt_str_at(row: &Row, idx: usize) -> Option<String> {
    row.try_get::<&str, _>(idx).ok().flatten().map(str::to_owned)
}

fn non_empty(v: Option<&str>) -> Option<&str> {
    v.map(str::trim).filter(|s| !s.is_empty())
}

/// Escapes LIKE wildcards so the user's text is matched literally.
fn escape_like(s: &str) -> String {
    s.replace('[', "[[]").replace('%', "[%]").replace('_', "[_]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_escaping() {
        assert_eq!(escape_like("usp_Get%[x]"), "usp[_]Get[%][[]x]");
    }

    #[test]
    fn non_empty_trims() {
        assert_eq!(non_empty(Some("  ")), None);
        assert_eq!(non_empty(Some(" a ")), Some("a"));
    }
}
