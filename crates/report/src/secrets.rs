//! What in a report's texts is shaped like a secret: API keys, `Bearer` tokens and long hex or base64 runs.

const REDACTED: &str = "<redacted>";

/// The prefixes of OpenAI's and Anthropic's API keys (`sk-`) and of GitHub's tokens.
const KEY_PREFIXES: [&str; 7] = ["sk-", "ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"];

/// The fewest characters after a key prefix that make a key, so that a word such as `sk-learn` stays.
const MIN_KEY_BODY: usize = 8;

/// The fewest characters of a hex or base64 run taken for a secret. Shorter ones are ids, symbol hashes and
/// words; a git commit hash (40) is redacted too.
const MIN_RUN: usize = 32;

/// `text` with every secret-shaped run of characters replaced by [`REDACTED`].
pub(crate) fn redact(text: &str) -> String {
  let mut redacted = String::with_capacity(text.len());
  let mut rest = text;
  while let Some(start) = rest.find(is_run_char) {
    redacted.push_str(&rest[..start]);
    let (run, after) = rest[start..].split_at(run_len(&rest[start..]));
    rest = after;
    if run.eq_ignore_ascii_case("bearer")
      && let Some((gap, token)) = bearer_token(after)
    {
      redacted.push_str(run);
      redacted.push_str(gap);
      redacted.push_str(REDACTED);
      rest = &after[gap.len() + token.len()..];
      continue;
    }
    redacted.push_str(if is_secret(run) { REDACTED } else { run });
  }
  redacted.push_str(rest);
  redacted
}

fn is_run_char(c: char) -> bool {
  c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '-')
}

/// The length of the run `text` starts with, with base64's `=` padding.
fn run_len(text: &str) -> usize {
  let body = text.find(|c| !is_run_char(c)).unwrap_or(text.len());
  let padding = text[body..].len() - text[body..].trim_start_matches('=').len();
  body + padding
}

/// The whitespace after the word `Bearer` and the token after it, when there is one.
fn bearer_token(after: &str) -> Option<(&str, &str)> {
  let token_start = after.find(|c: char| !c.is_whitespace())?;
  if token_start == 0 {
    return None;
  }
  let (gap, rest) = after.split_at(token_start);
  let token_len = rest.find(char::is_whitespace).unwrap_or(rest.len());
  Some((gap, &rest[..token_len]))
}

fn is_secret(run: &str) -> bool {
  let is_key = KEY_PREFIXES
    .iter()
    .any(|prefix| run.strip_prefix(prefix).is_some_and(|body| body.len() >= MIN_KEY_BODY));
  is_key || is_long_hex(run) || is_long_base64(run)
}

fn is_long_hex(run: &str) -> bool {
  run.len() >= MIN_RUN && run.bytes().all(|b| b.is_ascii_hexdigit())
}

/// A run of base64 is told from a path or a name by holding upper and lower case letters and digits.
fn is_long_base64(run: &str) -> bool {
  let body = run.trim_end_matches('=');
  body.len() >= MIN_RUN
    && body.bytes().any(|b| b.is_ascii_uppercase())
    && body.bytes().any(|b| b.is_ascii_lowercase())
    && body.bytes().any(|b| b.is_ascii_digit())
}
