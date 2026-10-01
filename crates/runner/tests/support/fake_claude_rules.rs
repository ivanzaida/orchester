//! How `orchester-fake-claude` decides a `Bash` call, the way Claude Code 2.1.89, 2.1.284 and 2.1.286 do:
//!
//! 1. deny rules, the whole tool or a `Bash(<prefix>:*)` prefix of the command, which win over every allow rule;
//! 2. a rule asking for the whole tool (`--settings` `permissions.ask`);
//! 3. a command with a variable expansion (`$VAR`) asks, whatever rule its words match;
//! 4. a compound command (`;`, `&&`, `||`, `|`) is decided part by part, and runs only when every part would;
//! 5. outside `acceptEdits`, the path check of a command that writes files, which 2.1.89 runs before the allow rules:
//!    it asks unless an `Edit(…)` allow rule covers the worktree;
//! 6. allow rules, by prefix;
//! 7. a few commands Claude Code classifies as read-only (`ls`, `cat`, `git status`, `git log`, `git diff`);
//! 8. `acceptEdits`, which runs `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp` and `sed` in any form;
//! 9. anything else asks. `dontAsk` denies what would ask, and so does print mode without a permission prompt tool.

/// Commands `acceptEdits` runs without a rule.
const MODE_ALLOWED: [&str; 7] = ["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"];
/// Commands the fake classifies as read-only, as Claude Code does with many more.
const READ_ONLY: [&str; 5] = ["ls", "cat", "git status", "git log", "git diff"];

/// What the CLI does with a `Bash` call of `command`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
  DeniedByRule,
  AllowedByRule,
  AllowedReadOnly,
  AllowedByMode,
  /// `dontAsk`, or print mode without a permission prompt: what would ask is denied.
  DeniedUnasked,
  Ask,
}

impl Decision {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::DeniedByRule => "deny-by-rule",
      Self::AllowedByRule => "allow-by-rule",
      Self::AllowedReadOnly => "allow-read-only",
      Self::AllowedByMode => "allow-by-mode",
      Self::DeniedUnasked => "deny-unasked",
      Self::Ask => "ask",
    }
  }

  pub const fn runs(self) -> bool {
    matches!(self, Self::AllowedByRule | Self::AllowedReadOnly | Self::AllowedByMode)
  }
}

pub fn bash(args: &[String], command: &str) -> Decision {
  let parts: Vec<&str> = command
    .split(['&', '|', ';'])
    .map(str::trim)
    .filter(|part| !part.is_empty())
    .collect();
  if parts.iter().any(|part| denied(args, part)) {
    return Decision::DeniedByRule;
  }
  if values(args, "--settings")
    .iter()
    .any(|settings| settings.contains(r#""ask":["Bash"]"#))
    || command.contains('$')
  {
    return unasked(args);
  }
  let decisions: Vec<Decision> = parts.iter().map(|part| simple(args, part)).collect();
  match decisions.as_slice() {
    [decision] => *decision,
    _ if decisions.iter().all(|decision| decision.runs()) => Decision::AllowedByRule,
    _ => unasked(args),
  }
}

fn denied(args: &[String], command: &str) -> bool {
  values(args, "--disallowedTools")
    .iter()
    .any(|rule| *rule == "Bash" || prefix_rule(rule).is_some_and(|prefix| covers(prefix, command)))
}

/// A command without operators.
fn simple(args: &[String], command: &str) -> Decision {
  let first = command.split(' ').next().unwrap_or_default();
  let allowed = values(args, "--allowedTools");
  let accepts_edits = values(args, "--permission-mode").contains(&"acceptEdits");
  let edits_by_rule = allowed.iter().any(|rule| rule.starts_with("Edit("));
  let writes = MODE_ALLOWED.contains(&first) && (first != "sed" || command.contains(" -i"));
  if writes && !accepts_edits && !edits_by_rule {
    return unasked(args);
  }
  if allowed
    .iter()
    .any(|rule| prefix_rule(rule).is_some_and(|prefix| covers(prefix, command)))
  {
    return Decision::AllowedByRule;
  }
  if READ_ONLY.iter().any(|read_only| covers(read_only, command)) {
    return Decision::AllowedReadOnly;
  }
  if accepts_edits && MODE_ALLOWED.contains(&first) {
    return Decision::AllowedByMode;
  }
  unasked(args)
}

/// `Ask`, which `dontAsk`, and print mode without a permission prompt tool, turn into a denial.
fn unasked(args: &[String]) -> Decision {
  let dont_ask = values(args, "--permission-mode").contains(&"dontAsk");
  if !dont_ask && args.iter().any(|arg| arg == "--permission-prompt-tool") {
    Decision::Ask
  } else {
    Decision::DeniedUnasked
  }
}

/// The prefix of a `Bash(<prefix>:*)` rule.
fn prefix_rule(rule: &str) -> Option<&str> {
  rule.strip_prefix("Bash(")?.strip_suffix(":*)")
}

/// Whether `prefix` matches `command` as Claude Code's prefix rules do: the same words, or more after them.
fn covers(prefix: &str, command: &str) -> bool {
  command == prefix || command.starts_with(&format!("{prefix} "))
}

/// The values after `flag`, up to the next option.
fn values<'a>(args: &'a [String], flag: &str) -> Vec<&'a str> {
  args
    .iter()
    .skip_while(|arg| *arg != flag)
    .skip(1)
    .take_while(|arg| !arg.starts_with("--"))
    .map(String::as_str)
    .collect()
}
