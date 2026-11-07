//! The words of this crate's [`Message`](crate::messages::Message)s and of the keys it hands out
//! (statuses, months, units): one JSON file per locale, which an app loads into its translations
//! under [`NAMESPACE`].

/// The namespace an app loads [`LOCALES`] under.
pub const NAMESPACE: &str = "workspace";

pub const FALLBACK_LOCALE: &str = "en";

/// `(locale, contents)` for every shipped locale. Adding a locale means adding its file here with
/// exactly the same keys and placeholders as `en.json`.
pub const LOCALES: &[(&str, &str)] = &[("en", include_str!("../assets/locales/en.json"))];

/// `message` in the fallback locale, for a message that carries other messages as lines of its own: a [`Message`]
/// holds text for its placeholders, not messages, so the lines are worded here and the app words the rest. A key the
/// locale lacks shows as the key itself.
pub fn fallback_text(message: &crate::messages::Message) -> String {
  static WORDS: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();
  let words = WORDS.get_or_init(|| {
    let contents = LOCALES
      .iter()
      .find(|(locale, _)| *locale == FALLBACK_LOCALE)
      .map_or("{}", |(_, contents)| contents);
    // The files are checked to be JSON by the `i18n` test; an unreadable one shows keys, which is visible.
    serde_json::from_str(contents).unwrap_or_default()
  });
  let Some(mut text) = message
    .key
    .split('.')
    .try_fold(words, |found, part| found.get(part))
    .and_then(serde_json::Value::as_str)
    .map(str::to_owned)
  else {
    return message.key.to_owned();
  };
  for (name, value) in &message.args {
    text = text.replace(&format!("{{{{{name}}}}}"), value);
  }
  text
}
