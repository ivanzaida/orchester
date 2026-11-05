//! A failed check's log tail as an agent may read it (`docs/design/pr-ci-status.md`, "Fix CI"): its terminal colour
//! codes stripped, at most [`LOG_TAIL_LINES`] lines and [`MOST_TAIL_CHARS`] characters, and everything shaped like a
//! secret replaced by [`REDACTED`] — keys and tokens by their known prefixes, a JWT, a private key block, a URL's user
//! part, the value of an assignment to a secret-sounding name, the token after `Bearer`, and long hex or base64 runs.
//! A CI log echoes what its job was given; a secret a workflow failed to mask must not reach a prompt.

use std::sync::LazyLock;

use regex::Regex;

use super::LOG_TAIL_LINES;
use crate::repository::redact_userinfo;

/// What a secret-shaped part of a log tail becomes.
pub const REDACTED: &str = "[redacted]";

/// Most characters of a log tail sent to an agent.
pub const MOST_TAIL_CHARS: usize = 4_000;

/// ANSI escape sequences: CSI (`ESC [ … final`) and OSC (`ESC ] … BEL` or `ESC \`).
static ANSI: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-_]").expect("valid"));

/// Keys and tokens by the prefixes their issuers give them, and JSON web tokens.
static KEYED: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(concat!(
    r"\b(?:sk|rk|pk)[-_](?:live_|test_|proj-|ant-)?[A-Za-z0-9_\-]{16,}",
    r"|\bgh[pousr]_[A-Za-z0-9]{20,}",
    r"|\bgithub_pat_[A-Za-z0-9_]{20,}",
    r"|\bglpat-[A-Za-z0-9_\-]{16,}",
    r"|\bxox[abposr]-[A-Za-z0-9\-]{10,}",
    r"|\b(?:AKIA|ASIA)[A-Z0-9]{16}\b",
    r"|\bAIza[A-Za-z0-9_\-]{30,}",
    r"|\bnpm_[A-Za-z0-9]{30,}",
    r"|\bpypi-[A-Za-z0-9_\-]{30,}",
    r"|\beyJ[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}\.[A-Za-z0-9_\-]{8,}",
  ))
  .expect("valid")
});

/// The value assigned to a secret-sounding name: `password=…`, `API_KEY: …`, `"token": "…"`.
static ASSIGNED: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(concat!(
    r#"(?i)(\b[A-Za-z0-9_.\-]*(?:password|passwd|pwd|secret|token|api[_-]?key|access[_-]?key|private[_-]?key|"#,
    r#"credentials?)[A-Za-z0-9_.\-]*["']?\s*[:=]\s*["']?)([^\s"',;]{4,})"#,
  ))
  .expect("valid")
});

/// The credentials after an authorization scheme.
static BEARER: LazyLock<Regex> =
  LazyLock::new(|| Regex::new(r"(?i)(\b(?:bearer|basic|token)\s+)([A-Za-z0-9_\-.=+/]{8,})").expect("valid"));

/// A long run of hex digits, or of base64 holding upper and lower case letters and digits.
static LONG_RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/_\-]{32,}={0,2}").expect("valid"));

/// The lines of a private key, from its `BEGIN` to its `END`.
static PRIVATE_KEY: LazyLock<Regex> = LazyLock::new(|| {
  Regex::new(r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?(?:-----END [A-Z ]*PRIVATE KEY-----|\z)").expect("valid")
});

/// `tail` as an agent may read it: colour codes stripped, cut to its last [`LOG_TAIL_LINES`] lines and
/// [`MOST_TAIL_CHARS`] characters, secrets redacted.
pub fn agent_log_tail(tail: &str) -> String {
  let plain = ANSI.replace_all(tail, "");
  let lines: Vec<&str> = plain.lines().collect();
  let kept = lines[lines.len().saturating_sub(LOG_TAIL_LINES)..].join("\n");
  let redacted = redact(&kept);
  let count = redacted.chars().count();
  if count <= MOST_TAIL_CHARS {
    return redacted;
  }
  // The end is where the failure is.
  redacted.chars().skip(count - MOST_TAIL_CHARS).collect()
}

/// `text` with everything shaped like a secret replaced by [`REDACTED`].
pub fn redact(text: &str) -> String {
  let text = PRIVATE_KEY.replace_all(text, REDACTED);
  let text = redact_userinfo(&text);
  let text = KEYED.replace_all(&text, REDACTED);
  let text = BEARER.replace_all(&text, format!("${{1}}{REDACTED}"));
  let text = ASSIGNED.replace_all(&text, |captures: &regex::Captures<'_>| {
    if captures[2].starts_with(REDACTED) {
      captures[0].to_owned()
    } else {
      format!("{}{REDACTED}", &captures[1])
    }
  });
  LONG_RUN
    .replace_all(&text, |captures: &regex::Captures<'_>| {
      let run = &captures[0];
      if is_secret_run(run) {
        REDACTED.to_owned()
      } else {
        run.to_owned()
      }
    })
    .into_owned()
}

/// The lengths of base64 secrets and of the lines they are wrapped at: an AWS secret access key (40), a PEM body line
/// (64), a MIME line (76).
const BASE64_SECRET_LENGTHS: [usize; 3] = [40, 64, 76];

/// A run is told from a path or a name by holding only hex digits, or upper and lower case letters and digits. A run
/// with `/` is a path unless it reads as base64: a `/`-free segment of 20 or more such characters, or a run of base64
/// characters only, not starting at a root, that is padded, holds a `+` or is as long as a base64 secret or its line.
fn is_secret_run(run: &str) -> bool {
  let body = run.trim_end_matches('=');
  if body.bytes().all(|byte| byte.is_ascii_hexdigit()) {
    return true;
  }
  if !is_mixed(body) {
    return false;
  }
  if !body.contains('/') {
    return true;
  }
  let random_segment = body.split('/').any(|segment| segment.len() >= 20 && is_mixed(segment));
  let base64 = body
    .bytes()
    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/');
  let shaped = body.len() < run.len() || body.contains('+') || BASE64_SECRET_LENGTHS.contains(&run.len());
  random_segment || (base64 && !body.starts_with('/') && shaped)
}

/// `text` holds upper and lower case letters and digits.
fn is_mixed(text: &str) -> bool {
  text.bytes().any(|byte| byte.is_ascii_uppercase())
    && text.bytes().any(|byte| byte.is_ascii_lowercase())
    && text.bytes().any(|byte| byte.is_ascii_digit())
}
