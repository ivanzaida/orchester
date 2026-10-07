//! A native agent's command (`docs/architecture/native-agent.md` §4.6), read with the grammar of what
//! actually runs it. A shell command on Windows goes to PowerShell, which keeps a backslash in a path
//! where Bash would drop it; until a PowerShell grammar covers the general case only a known-safe read
//! (a process listing, a reading cmdlet with its paths placed) keeps its category there, and any other
//! part is Unknown, so no rule answers it and auto mode's hard rules hold. A command started without a
//! shell is classified from its arguments as the platform split them.

use std::path::Path;

use super::{
  ClassifiedCommand, CommandCategory, CommandPart, CommandScope, Quoting, RawPart, classify_part, shell_parts,
};

/// How a native shell command reaches PowerShell (`orchester_core::run::CommandShell::invocation`).
const POWERSHELL: [&str; 4] = ["pwsh", "-NoProfile", "-NonInteractive", "-Command"];

impl ClassifiedCommand {
  /// `script` as `pwsh -NoProfile -NonInteractive -Command` (or Windows PowerShell 5.1) runs it in
  /// `folder`, placed in `scope`.
  pub fn classify_powershell_in(script: &str, folder: &Path, scope: &CommandScope) -> Self {
    let words = POWERSHELL.iter().map(|word| (*word).to_owned()).collect();
    let mut command = Self::read_in(script, folder, scope, |line| {
      let inline = Some((script.to_owned(), Quoting::PowerShell));
      shell_parts(&RawPart::default(), words, inline, scope, line, 0)
    });
    for part in command.parts.iter_mut().filter(|part| !part.known_safe_read()) {
      part.at_least(CommandCategory::Unknown);
    }
    command
  }

  /// The program and arguments `argv`, started without a shell in `folder`, placed in `scope`; `text` is
  /// the command as it was asked about.
  pub fn classify_argv_in(text: &str, argv: &[String], folder: &Path, scope: &CommandScope) -> Self {
    let raw = RawPart {
      words: argv.to_vec(),
      // No shell reads a variable assignment or a benign prefix in it.
      quoted_name: true,
      ..RawPart::default()
    };
    Self::read_in(text, folder, scope, |line| {
      classify_part(&raw, Quoting::Bash, scope, line, 0)
    })
  }
  /// `text`, which cannot be split into a program and its arguments: Unknown.
  pub fn unread(text: &str) -> Self {
    Self {
      parts: vec![CommandPart::unknown(vec![text.to_owned()])],
      text: text.to_owned(),
    }
  }
}
