//! Translation resources: one file per locale and area of the app (`shell`, `common`, `task`, a screen's
//! own), `assets/locales/<locale>/<area>.json`. An area's file holds its keys under the area's name and
//! nothing else (`tasks.json` is `{"tasks": {…}}`), so no key is in two files and each area's strings
//! have one owner; `tests/i18n.rs` checks it. `build.rs` lists the files, so an area is added by adding
//! its file. Every area merges into lurq's default namespace, where code names a key in full
//! (`ctx.t("shell.screens.tasks")`). See the `orchester-ui` skill for key naming and locale rules.

use lurq::app::{App, i18n::LoadJsonError};

pub const FALLBACK_LOCALE: &str = "en";

/// A shipped locale: its name and its areas' files. Every locale has exactly the areas, keys and
/// placeholders of the fallback locale.
pub struct Locale {
  pub name: &'static str,
  pub areas: &'static [Area],
}

/// One area's strings in one locale: the area's name (its file's, and its keys' first segment) and the
/// file's JSON.
pub struct Area {
  pub name: &'static str,
  pub contents: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// A bundled locale file that lurq cannot load.
#[derive(Debug, thiserror::Error)]
#[error("the `{area}` strings of locale `{locale}` do not load")]
pub struct LocaleError {
  locale: &'static str,
  area: &'static str,
  #[source]
  source: LoadJsonError,
}

pub fn install(app: &App) -> Result<(), LocaleError> {
  let i18n = app.i18n();
  for locale in LOCALES {
    for area in locale.areas {
      i18n
        .add_json(locale.name, "translation", area.contents)
        .map_err(|source| LocaleError {
          locale: locale.name,
          area: area.name,
          source,
        })?;
    }
  }
  i18n.set_fallback_locale(FALLBACK_LOCALE);
  i18n.set_locale(FALLBACK_LOCALE);
  Ok(())
}
