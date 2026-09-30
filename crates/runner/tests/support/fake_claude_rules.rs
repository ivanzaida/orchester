//! How `orchester-fake-claude` decides a `Bash` call before any prompt, the way Claude Code 2.1.89
//! and 2.1.284 do: deny rules first, then a rule asking for the whole tool (`--settings`
//! `permissions.ask`), then allow rules, then `acceptEdits`, which runs `mkdir`, `touch`, `rm`,
//! `rmdir`, `mv`, `cp` and `sed` without a rule; anything else asks.

/// Commands `acceptEdits` runs without a rule.
const MODE_ALLOWED: [&str; 7] = ["mkdir", "touch", "rm", "rmdir", "mv", "cp", "sed"];

/// What the CLI does with a `Bash` call of `command`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
  DeniedByRule,
  AllowedByRule,
  AllowedByMode,
  Ask,
}

impl Decision {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::DeniedByRule => "deny-by-rule",
      Self::AllowedByRule => "allow-by-rule",
      Self::AllowedByMode => "allow-by-mode",
      Self::Ask => "ask",
    }
  }
}

pub fn bash(args: &[String], command: &str) -> Decision {
  let first = command.split(' ').next().unwrap_or_default();
  let denied = values(args, "--disallowedTools");
  if denied
    .iter()
    .any(|rule| *rule == "Bash" || *rule == format!("Bash({first}:*)"))
  {
    return Decision::DeniedByRule;
  }
  if values(args, "--settings")
    .iter()
    .any(|settings| settings.contains(r#""ask":["Bash"]"#))
  {
    return Decision::Ask;
  }
  let allowed = values(args, "--allowedTools").into_iter().any(|rule| {
    rule
      .strip_prefix("Bash(")
      .and_then(|rule| rule.strip_suffix(":*)"))
      .is_some_and(|prefix| command == prefix || command.starts_with(&format!("{prefix} ")))
  });
  if allowed {
    return Decision::AllowedByRule;
  }
  let accepts_edits = values(args, "--permission-mode").contains(&"acceptEdits");
  if accepts_edits && MODE_ALLOWED.contains(&first) {
    return Decision::AllowedByMode;
  }
  Decision::Ask
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
