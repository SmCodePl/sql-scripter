//! Structured connection parameters. The connection string is derived from these fields
//! (read-only, password masked) and the TDS client is configured directly from them.

use serde::{Deserialize, Serialize};
use tiberius::{AuthMethod, Config, EncryptionLevel};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AuthKind {
    /// SQL Server login (user + password)
    #[default]
    Sql,
    /// Windows integrated authentication (Windows only)
    Windows,
}

impl AuthKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthKind::Sql => "sql",
            AuthKind::Windows => "windows",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("windows") { AuthKind::Windows } else { AuthKind::Sql }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionParams {
    /// Host name or IP; may include a named instance: `HOST\SQLEXPRESS`.
    pub server: String,
    /// TCP port; `None` = 1433 (or SQL Browser lookup for named instances).
    pub port: Option<u16>,
    pub database: Option<String>,
    pub auth: AuthKind,
    pub username: Option<String>,
    /// TLS for the whole session (`Encrypt=true`). When false only the login packet is encrypted.
    pub encrypt: bool,
    pub trust_server_certificate: bool,
}

const MASK: &str = "********";

impl ConnectionParams {
    pub fn validate(&self) -> Result<()> {
        if self.server.trim().is_empty() {
            return Err(Error::validation("Serwer jest wymagany", "Server is required"));
        }
        if self.auth == AuthKind::Sql && self.username.as_deref().is_none_or(|u| u.trim().is_empty()) {
            return Err(Error::validation("Użytkownik jest wymagany dla logowania SQL Server", "User is required for SQL Server authentication"));
        }
        Ok(())
    }

    fn host_and_instance(&self) -> (&str, Option<&str>) {
        let server = self.server.trim();
        let server = server.strip_prefix("tcp:").unwrap_or(server);
        match server.split_once('\\') {
            Some((host, instance)) if !instance.is_empty() => (host, Some(instance)),
            _ => (server, None),
        }
    }

    /// ADO.NET connection string for display/copy. The password is always masked.
    pub fn display_string(&self, has_password: bool) -> String {
        let mut parts = Vec::new();
        let mut server = self.server.trim().to_owned();
        if let Some(port) = self.port {
            server.push_str(&format!(",{port}"));
        }
        parts.push(format!("Server={}", quote_value(&server)));
        if let Some(db) = self.database.as_deref().filter(|d| !d.trim().is_empty()) {
            parts.push(format!("Database={}", quote_value(db.trim())));
        }
        match self.auth {
            AuthKind::Windows => parts.push("Integrated Security=true".into()),
            AuthKind::Sql => {
                parts.push(format!("User Id={}", quote_value(self.username.as_deref().unwrap_or("").trim())));
                parts.push(format!("Password={}", if has_password { MASK } else { "" }));
            }
        }
        parts.push(format!("Encrypt={}", self.encrypt));
        parts.push(format!("TrustServerCertificate={}", self.trust_server_certificate));
        parts.join(";")
    }

    /// Builds the tiberius config directly from the fields (no string parsing, so passwords
    /// with `;` or quotes are safe).
    pub fn to_config(&self, password: Option<&str>) -> Result<Config> {
        self.validate()?;
        let mut config = Config::new();
        let (host, instance) = self.host_and_instance();
        config.host(host);
        if let Some(instance) = instance {
            config.instance_name(instance);
        }
        if let Some(port) = self.port {
            config.port(port);
        }
        if let Some(db) = self.database.as_deref().filter(|d| !d.trim().is_empty()) {
            config.database(db.trim());
        }
        config.application_name("SQL Scripter");
        config.encryption(if self.encrypt { EncryptionLevel::Required } else { EncryptionLevel::Off });
        if self.trust_server_certificate {
            config.trust_cert();
        }

        match self.auth {
            AuthKind::Sql => config.authentication(AuthMethod::sql_server(
                self.username.as_deref().unwrap_or("").trim(),
                password.unwrap_or(""),
            )),
            #[cfg(windows)]
            AuthKind::Windows => config.authentication(AuthMethod::Integrated),
            #[cfg(not(windows))]
            AuthKind::Windows => {
                return Err(Error::validation(
                    "Uwierzytelnianie Windows działa tylko w systemie Windows",
                    "Windows authentication is only available on Windows",
                ));
            }
        }
        Ok(config)
    }

    /// Parses a legacy ADO.NET connection string (used when migrating the v1 store).
    /// Returns the params and the plaintext password, if any.
    pub fn from_ado_string(cs: &str) -> (Self, Option<String>) {
        let mut params = ConnectionParams { encrypt: true, ..Default::default() };
        let mut password = None;

        for (key, value) in split_ado(cs) {
            match key.as_str() {
                "server" | "data source" | "address" | "addr" | "network address" => {
                    let v = value.strip_prefix("tcp:").unwrap_or(&value);
                    match v.rsplit_once(',') {
                        Some((host, port)) if port.trim().parse::<u16>().is_ok() => {
                            params.server = host.trim().to_owned();
                            params.port = port.trim().parse().ok();
                        }
                        _ => params.server = v.trim().to_owned(),
                    }
                }
                "database" | "initial catalog" => params.database = Some(value),
                "user id" | "uid" | "user" | "username" => params.username = Some(value),
                "password" | "pwd" => password = Some(value),
                "integrated security" | "trusted_connection" => {
                    if is_true(&value) {
                        params.auth = AuthKind::Windows;
                    }
                }
                "encrypt" => params.encrypt = is_true(&value) || value.eq_ignore_ascii_case("strict"),
                "trustservercertificate" | "trust server certificate" => params.trust_server_certificate = is_true(&value),
                _ => {}
            }
        }
        (params, password.filter(|p| !p.is_empty()))
    }
}

fn is_true(v: &str) -> bool {
    matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "yes" | "sspi" | "1")
}

/// Quotes a value when it contains characters with special meaning in a connection string.
fn quote_value(v: &str) -> String {
    if v.contains([';', '=', '"', '\'']) || v != v.trim() {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_owned()
    }
}

/// Splits `key=value;...` honouring `"..."` / `'...'` quoted values with doubled quotes.
fn split_ado(cs: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut chars = cs.chars().peekable();
    loop {
        let key: String = chars.by_ref().take_while(|&c| c != '=').collect();
        if key.trim().is_empty() && chars.peek().is_none() {
            break;
        }
        let mut value = String::new();
        while chars.peek().is_some_and(|c| c.is_whitespace()) {
            chars.next();
        }
        match chars.peek().copied() {
            Some(q @ ('"' | '\'')) => {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == q {
                        if chars.peek() == Some(&q) {
                            chars.next();
                            value.push(q);
                        } else {
                            break;
                        }
                    } else {
                        value.push(c);
                    }
                }
                // Skip anything up to the separator.
                for c in chars.by_ref() {
                    if c == ';' {
                        break;
                    }
                }
            }
            _ => {
                for c in chars.by_ref() {
                    if c == ';' {
                        break;
                    }
                    value.push(c);
                }
                value = value.trim().to_owned();
            }
        }
        if !key.trim().is_empty() {
            out.push((key.trim().to_ascii_lowercase(), value));
        }
        if chars.peek().is_none() {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_legacy_strings() {
        let (p, pw) = ConnectionParams::from_ado_string(
            "Server=tcp:sqlprod01,1433;Initial Catalog=Sales;User Id=app;Password=\"a;b\"\"c\";TrustServerCertificate=True;Encrypt=false",
        );
        assert_eq!(p.server, "sqlprod01");
        assert_eq!(p.port, Some(1433));
        assert_eq!(p.database.as_deref(), Some("Sales"));
        assert_eq!(p.username.as_deref(), Some("app"));
        assert_eq!(pw.as_deref(), Some("a;b\"c"));
        assert!(p.trust_server_certificate);
        assert!(!p.encrypt);
        assert_eq!(p.auth, AuthKind::Sql);

        let (p, pw) = ConnectionParams::from_ado_string("Data Source=HOST\\SQLEXPRESS;Database=X;Integrated Security=SSPI");
        assert_eq!(p.server, "HOST\\SQLEXPRESS");
        assert_eq!(p.port, None);
        assert_eq!(p.auth, AuthKind::Windows);
        assert!(pw.is_none());
        assert!(p.encrypt, "encrypt defaults to true");
    }

    #[test]
    fn display_string_masks_password_and_quotes() {
        let p = ConnectionParams {
            server: "db.local".into(),
            port: Some(14330),
            database: Some("My;Db".into()),
            auth: AuthKind::Sql,
            username: Some("sa".into()),
            encrypt: true,
            trust_server_certificate: false,
        };
        assert_eq!(
            p.display_string(true),
            "Server=db.local,14330;Database=\"My;Db\";User Id=sa;Password=********;Encrypt=true;TrustServerCertificate=false"
        );
        assert!(p.display_string(false).contains("Password=;"));
        // Round-trips through the legacy parser.
        let (back, _) = ConnectionParams::from_ado_string(&p.display_string(true));
        assert_eq!(back, p);
    }

    #[test]
    fn validates_and_builds_config() {
        let mut p = ConnectionParams { server: "HOST\\INST".into(), username: Some("sa".into()), encrypt: true, ..Default::default() };
        let config = p.to_config(Some("x;y")).unwrap();
        // Named instance: tiberius goes through SQL Browser (UDP 1434) to find the port.
        assert_eq!(config.get_addr(), "HOST:1434");

        p.username = None;
        assert!(matches!(p.to_config(None), Err(Error::Validation(_))));
        p.server = " ".into();
        assert!(matches!(p.validate(), Err(Error::Validation(_))));
    }
}
