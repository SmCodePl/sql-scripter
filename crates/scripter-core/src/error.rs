use std::fmt;
use std::time::Duration;

use crate::i18n::t;

/// Errors surfaced to the UI. `Display` follows the current UI language (see [`crate::i18n`]).
#[derive(Debug)]
pub enum Error {
    Store(sqlx::Error),
    Mssql(tiberius::error::Error),
    Io(std::io::Error),
    Timeout(Duration),
    /// Password could not be encrypted/decrypted (wrong or missing key).
    Crypto(String),
    /// Already translated, user-facing message.
    Validation(String),
    NotFound(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Store(e) => write!(f, "{}: {e}", t("Błąd lokalnej bazy połączeń (SQLite)", "Local connection store error (SQLite)")),
            Error::Mssql(e) => write!(f, "{}: {e}", t("Błąd SQL Server", "SQL Server error")),
            Error::Io(e) => write!(f, "{}: {e}", t("Błąd zapisu/odczytu pliku", "File I/O error")),
            Error::Timeout(d) => write!(f, "{} ({}s)", t("Przekroczono limit czasu połączenia", "Connection timed out"), d.as_secs()),
            Error::Crypto(e) => write!(f, "{}: {e}", t("Błąd szyfrowania hasła", "Password encryption error")),
            Error::Validation(m) => f.write_str(m),
            Error::NotFound(what) => write!(f, "{}: {what}", t("Nie znaleziono", "Not found")),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Store(e) => Some(e),
            Error::Mssql(e) => Some(e),
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(e: sqlx::Error) -> Self {
        Error::Store(e)
    }
}

impl From<tiberius::error::Error> for Error {
    fn from(e: tiberius::error::Error) -> Self {
        Error::Mssql(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl Error {
    /// True when the TDS connection is most likely dead and a reconnect is worth trying.
    pub fn is_connection_lost(&self) -> bool {
        matches!(self, Error::Io(_) | Error::Mssql(tiberius::error::Error::Io { .. }))
    }

    pub fn validation(pl: &'static str, en: &'static str) -> Self {
        Error::Validation(t(pl, en).to_owned())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
