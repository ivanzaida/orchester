//! Why a command asks, as the approval dialog says it under the category (`NV6xJ`, `vyShL`): what the
//! part that names the command does, and the path, package or program it does it to, as written. The
//! dialog words each reason; core only says which it is.

use super::{ClassifiedCommand, CommandCategory, CommandPart};

/// What a command does that makes it ask, most important first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommandReason {
  /// Orchester could not read it to the end.
  Unknown,
  /// It deletes `path` outside the worktree and the temporary folder.
  DeletesOutside { path: String },
  /// It rewrites or discards history (`command` is its program and subcommand: `git push`).
  RewritesHistory { command: String },
  /// It reads a credential location (`~/.ssh/id_rsa`) or a credential store (`security`).
  ReadsCredentials { what: String },
  /// It installs software from `registry` (`crates.io`), naming `package` first when it names one.
  Installs { registry: String, package: Option<String> },
  /// It writes `path` outside the worktree.
  WritesOutside { path: String },
  /// It reaches the network (`command`: `git fetch`, `curl`).
  Network { command: String },
  /// It changes the worktree (`command`: `cargo test`).
  Worktree { command: String },
  /// It only reads (`command`: `git status`).
  ReadOnly { command: String },
}

impl ClassifiedCommand {
  /// Why this command asks; `None` for a command with no part.
  pub fn reason(&self) -> Option<CommandReason> {
    let forcing = self.forcing()?;
    let secret = self.parts().iter().find(|part| part.reads_secrets());
    let install = self.parts().iter().find_map(|part| part.source.clone());
    Some(match (forcing.category(), secret, install) {
      (CommandCategory::Unknown, ..) => CommandReason::Unknown,
      (CommandCategory::HistoryDestructive, ..) => match &forcing.outside_delete {
        Some(path) => CommandReason::DeletesOutside { path: path.clone() },
        None => CommandReason::RewritesHistory {
          command: forcing.named(),
        },
      },
      (_, Some(part), _) => CommandReason::ReadsCredentials {
        what: part.secret_word.clone().unwrap_or_else(|| part.named()),
      },
      (_, None, Some((registry, package))) => CommandReason::Installs { registry, package },
      (CommandCategory::OutsideWrite, ..) => CommandReason::WritesOutside {
        path: forcing.outside_write.clone().unwrap_or_else(|| forcing.named()),
      },
      (CommandCategory::Network, ..) => CommandReason::Network {
        command: forcing.named(),
      },
      (CommandCategory::Worktree, ..) => CommandReason::Worktree {
        command: forcing.named(),
      },
      (CommandCategory::ReadOnly, ..) => CommandReason::ReadOnly {
        command: forcing.named(),
      },
    })
  }
}

impl CommandPart {
  /// The program and subcommand as a person names them (`cargo add`), else the first word.
  fn named(&self) -> String {
    match (&self.program, &self.subcommand) {
      (Some(program), Some((_, sub))) => format!("{program} {sub}"),
      (Some(program), None) => program.clone(),
      (None, _) => self.words.first().cloned().unwrap_or_default(),
    }
  }
}
