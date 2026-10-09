//! Translation resources. Every user-facing string is a key in these files; see the
//! `orchester-ui` skill for key naming and locale rules.

use lurq::app::{App, i18n::LoadJsonError};

pub const FALLBACK_LOCALE: &str = "en";

/// `(locale, contents)` for every shipped locale. Adding a locale means adding its file
/// here with exactly the same keys and placeholders as `en.json`.
pub const LOCALES: &[(&str, &str)] = &[("en", include_str!("../assets/locales/en.json"))];

pub fn install(app: &App) -> Result<(), LoadJsonError> {
  let i18n = app.i18n();
  for (locale, contents) in LOCALES {
    i18n.add_json(*locale, "translation", *contents)?;
  }
  i18n.set_fallback_locale(FALLBACK_LOCALE);
  i18n.set_locale(FALLBACK_LOCALE);
  Ok(())
}
