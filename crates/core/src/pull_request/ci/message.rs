//! What Fix CI tells the run's agent (`docs/design/pr-ci-status.md`, "Fix CI"): one message with the lead, each failing
//! check's name linked to its page with its log tail ([`super::redact::agent_log_tail`]) in a fenced block, and the
//! ask. Passed and skipped checks are left out. Agents read it as written, like a feedback request
//! ([`crate::pull_request::feedback::request_text`]), so it is not translated; the person sees the same Markdown as
//! their message, drawn as the chat draws an agent's (`11b` `X8xYZ`).

use std::fmt::Write as _;

use super::{
  CiChecks, CiState,
  fix::{FixedBy, is_failing},
  redact::agent_log_tail,
};

/// The Fix CI message for the failing checks of pull request `number`, whose head is `branch`, sent `by`; `None` when
/// no check fails.
pub fn fix_text(checks: &CiChecks, (number, branch): (u64, &str), by: FixedBy) -> Option<String> {
  let failing: Vec<_> = checks.checks.iter().filter(|check| is_failing(check.state)).collect();
  if failing.is_empty() {
    return None;
  }
  let (count, total) = (failing.len(), checks.checks.len());
  let mut text = match by {
    FixedBy::Hand => format!("CI failed on pull request #{number}. {count} of {total} checks failed:\n"),
    FixedBy::Automatically { count: 1, limit } => format!(
      "CI failed on pull request #{number}. This is Fix CI 1 of {}; {count} of {total} checks failed:\n",
      limit.get()
    ),
    FixedBy::Automatically { count: fix, limit } => format!(
      "CI failed on pull request #{number} again. This is Fix CI {fix} of {}; {count} of {total} checks failed:\n",
      limit.get()
    ),
  };
  for check in failing {
    let state = if check.state == CiState::Cancelled {
      "cancelled"
    } else {
      "failed"
    };
    text.push('\n');
    let name = link_text(&check.name);
    let name = match &check.url {
      Some(url) => format!("[{name}]({})", destination(url)),
      None => name,
    };
    match &check.log_tail {
      Some(_) => writeln!(text, "{name} · {state} · last lines of the log"),
      None => writeln!(text, "{name} · {state} · no log could be read"),
    }
    .expect("writing to a String does not fail");
    if let Some(tail) = &check.log_tail {
      let tail = agent_log_tail(tail);
      let fence = fence_for(&tail);
      writeln!(text, "{fence}\n{tail}\n{fence}").expect("writing to a String does not fail");
    }
  }
  let ask = if count == 1 {
    format!("\nFix it on {branch} and push. Don't change the test's expectations unless they are wrong.")
  } else {
    format!("\nFix them on {branch} and push. Don't change the tests' expectations unless they are wrong.")
  };
  text.push_str(&ask);
  Some(text)
}

/// A code fence longer than any run of backticks in `text`, so the tail cannot end its block.
fn fence_for(text: &str) -> String {
  let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
  "`".repeat(longest.max(2) + 1)
}

/// A check's name as Markdown that shows it as written: the characters that would make it a link, code or emphasis
/// escaped.
fn link_text(name: &str) -> String {
  let mut text = String::with_capacity(name.len());
  for ch in name.chars() {
    if matches!(ch, '\\' | '[' | ']' | '`' | '*' | '_' | '<' | '>') {
      text.push('\\');
    }
    text.push(ch);
  }
  text
}

/// `url` as a Markdown link's destination: in angle brackets when it holds a space or a parenthesis.
fn destination(url: &str) -> String {
  if url.contains(|ch: char| ch.is_whitespace() || matches!(ch, '(' | ')' | '<' | '>')) {
    format!("<{}>", url.replace(['<', '>'], ""))
  } else {
    url.to_owned()
  }
}
