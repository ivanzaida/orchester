//! Translation resources. Every user-facing string is a key in these files or in
//! `orchester-workspace`'s; see the `orchester-ui` skill for key naming and locale rules.

use lurq::app::{App, i18n::LoadJsonError};
use orchester_workspace::i18n as workspace;

pub const FALLBACK_LOCALE: &str = "en";

/// `(locale, contents)` for every shipped locale. Adding a locale means adding its file
/// here with exactly the same keys and placeholders as `en.json`.
pub const LOCALES: &[(&str, &str)] = &[("en", include_str!("../assets/locales/en.json"))];

/// The namespace `t` and `t_args` read.
const DEFAULT_NAMESPACE: &str = "translation";

pub fn install(app: &App) -> Result<(), LoadJsonError> {
  let i18n = app.i18n();
  for (locale, contents) in LOCALES {
    i18n.add_json(*locale, DEFAULT_NAMESPACE, *contents)?;
  }
  for (locale, contents) in workspace::LOCALES {
    i18n.add_json(*locale, workspace::NAMESPACE, *contents)?;
    // The screens were written before the workspace's words had a file of their own: they translate
    // its keys, and its messages, with `t`. The two files share no key.
    i18n.add_json(*locale, DEFAULT_NAMESPACE, *contents)?;
  }
  i18n.set_fallback_locale(FALLBACK_LOCALE);
  i18n.set_locale(FALLBACK_LOCALE);
  Ok(())
}
