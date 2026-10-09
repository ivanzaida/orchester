//! The words of this crate's [`Message`](crate::messages::Message)s and of the keys it hands out
//! (statuses, months, units): one JSON file per locale, which an app loads into its translations
//! under [`NAMESPACE`].

/// The namespace an app loads [`LOCALES`] under.
pub const NAMESPACE: &str = "workspace";

pub const FALLBACK_LOCALE: &str = "en";

/// `(locale, contents)` for every shipped locale. Adding a locale means adding its file here with
/// exactly the same keys and placeholders as `en.json`.
pub const LOCALES: &[(&str, &str)] = &[("en", include_str!("../assets/locales/en.json"))];
