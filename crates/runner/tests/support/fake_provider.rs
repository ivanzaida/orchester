//! What a fake CLI reports of the API provider it was started on (`docs/design/api-providers.md`): where it would
//! send its requests and the key, on standard error. The key is printed on purpose: the runner must keep it out of
//! the activity, so a test sees it redacted.

#![allow(dead_code, reason = "each fake CLI uses the part about itself")]

/// Reports `provider`, how the CLI was pointed at the provider's API, if it was, and the key in variable `key`.
pub fn report(provider: Option<String>, key: &str) {
  if let Some(provider) = provider {
    eprintln!("provider: {provider}");
  }
  if let Ok(value) = std::env::var(key) {
    eprintln!("provider key: {value}");
  }
}

/// The model provider Codex was told to run on by its arguments, `openai` when none.
pub fn codex_provider(args: &[String]) -> String {
  args
    .iter()
    .find_map(|arg| arg.strip_prefix("model_provider="))
    .map_or_else(|| "openai".to_owned(), |provider| provider.trim_matches('"').to_owned())
}

/// Reports Claude Code's provider variables: the base URL and the models of its aliases and subagents.
pub fn claude() {
  let models = ["ANTHROPIC_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "CLAUDE_CODE_SUBAGENT_MODEL"]
    .map(|name| std::env::var(name).unwrap_or_default());
  let provider = std::env::var("ANTHROPIC_BASE_URL")
    .ok()
    .map(|url| format!("{url} {}", models.join(" ")));
  report(provider, "ANTHROPIC_AUTH_TOKEN");
}
