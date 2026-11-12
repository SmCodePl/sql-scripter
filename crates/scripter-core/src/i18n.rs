//! Minimal backend i18n: the UI language is mirrored here so error messages match it.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Pl,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::Pl => "pl",
            Lang::En => "en",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "pl" => Some(Lang::Pl),
            "en" => Some(Lang::En),
            _ => None,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    if CURRENT.load(Ordering::Relaxed) == Lang::En as u8 { Lang::En } else { Lang::Pl }
}

/// Picks the Polish or English text for the current language.
pub fn t(pl: &'static str, en: &'static str) -> &'static str {
    match lang() {
        Lang::Pl => pl,
        Lang::En => en,
    }
}
