//! How `orchester-fake-claude` decides a `Bash` call, the way Claude Code 2.1.89 and 2.1.284 do:
//!
//! 1. deny rules, the whole tool or a `Bash(<prefix>:*)` prefix of the command, which win over every allow rule;
//! 2. a rule asking for the whole tool (`--settings` `permissions.ask`);
//! 3. outside `acceptEdits`, the path check of a command that writes files, which 2.1.89 runs before the allow rules:
//!    it asks unless an `Edit(…)` allow rule covers the worktree;
//! 4. allow rules, by prefix;
//! 5. `acceptEdits`, which runs `mkdir`, `touch`, `rm`, `rmdir`, `mv`, `cp` and `sed` in any form;
//! 6. anything else asks, and print mode denies what it would ask unless the invocation names a permission prompt tool.

/// Commands `acceptEdits` runs without a rule.
const MODE_ALLOWED: [&str; 7] = ["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"];

/// What the CLI does with a `Bash` call of `command`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
  DeniedByRule,
  AllowedByRule,
  AllowedByMode,
  /// Print mode without a permission prompt: what would ask is denied.
  DeniedUnasked,
  Ask,
}

impl Decision {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::DeniedByRule => "deny-by-rule",
      Self::AllowedByRule => "allow-by-rule",
      Self::AllowedByMode => "allow-by-mode",
      Self::DeniedUnasked => "deny-unasked",
      Self::Ask => "ask",
    }
  }
}

pub fn bash(args: &[String], command: &str) -> Decision {
  let first = command.split(' ').next().unwrap_or_default();
  let denied = values(args, "--disallowedTools");
  if denied
    .iter()
    .any(|rule| *rule == "Bash" || prefix_rule(rule).is_some_and(|prefix| covers(prefix, command)))
  {
    return Decision::DeniedByRule;
  }
  if values(args, "--settings")
    .iter()
    .any(|settings| settings.contains(r#""ask":["Bash"]"#))
  {
    return Decision::Ask;
  }
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
  if accepts_edits && MODE_ALLOWED.contains(&first) {
    return Decision::AllowedByMode;
  }
  unasked(args)
}

/// `Ask`, which print mode turns into a denial unless a permission prompt tool answers it.
fn unasked(args: &[String]) -> Decision {
  if args.iter().any(|arg| arg == "--permission-prompt-tool") {
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
