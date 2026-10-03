//! Command categories (`docs/design/orchester-workspace.md`, "Command categories and approval policy",
//! board `U97QE`): a command an agent's CLI asks about is split into its simple commands, each is
//! classified by its program, subcommand, options and the paths it names, and the strictest part
//! names the command. Only what is understood is classified: an unknown program or subcommand, an
//! option that is not listed for its program, a variable set before a program (`PATH=…`,
//! `LD_PRELOAD=…`, `GIT_…=…`) and anything not read to the end are Unknown, so they ask.
//!
//! Run temp (a temporary file this run created) is not told apart yet: a temporary file is outside
//! the worktree like any other. Besides its category a part says whether it reads a credential
//! location ([`CommandScope`]) and whether it installs software, which auto-approval never approves
//! ([`crate::run::auto_verdict`]).

mod files;
mod gh;
mod git;
mod install;
mod network;
mod network_tables;
mod options;
mod packages;
mod part;
mod paths;
mod place;
mod powershell;
mod programs;
mod reason;
mod words;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use self::{
  part::{CommandPart, SimilarCommand},
  paths::CommandScope,
  reason::CommandReason,
};
use self::{
  place::{Folders, Placing},
  programs::{Judged, Kind},
  words::{Quoting, RawPart},
};
use super::known_safe::program_name;

/// How deep shells that run a script (`bash -c`, `pwsh -Command`) are followed.
const SHELL_DEPTH: usize = 3;

/// What a command does, from the least to the most strict; on a tie the stricter category names the
/// command, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandCategory {
  /// Reads files, history and state; changes nothing.
  ReadOnly,
  /// Writes, moves or deletes only inside the worktree (and its extra writable folders).
  Worktree,
  /// Reaches the network.
  Network,
  /// Writes or deletes outside the worktree, the temporary folder included.
  OutsideWrite,
  /// Not read to the end: a script, a variable, a substitution, a program or option Orchester does
  /// not know.
  Unknown,
  /// Rewrites or discards history, or deletes outside the temporary folder.
  HistoryDestructive,
}

impl CommandCategory {
  pub const ALL: [Self; 6] = [
    Self::ReadOnly,
    Self::Worktree,
    Self::Network,
    Self::OutsideWrite,
    Self::Unknown,
    Self::HistoryDestructive,
  ];

  /// How the category is stored.
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::ReadOnly => "read_only",
      Self::Worktree => "worktree",
      Self::Network => "network",
      Self::OutsideWrite => "outside_write",
      Self::Unknown => "unknown",
      Self::HistoryDestructive => "history_destructive",
    }
  }
}

/// A command line split into its simple commands, each classified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassifiedCommand {
  parts: Vec<CommandPart>,
}

impl ClassifiedCommand {
  /// `command` as the agent's CLI asked about it, run in `scope`'s worktree.
  pub fn classify(command: &str, scope: &CommandScope) -> Self {
    Self::classify_in(command, scope.worktree(), scope)
  }

  /// `command` run in `folder` (the working folder the agent's CLI reported), placed in `scope`.
  pub fn classify_in(command: &str, folder: &Path, scope: &CommandScope) -> Self {
    let mut folders = Some(vec![scope.worktree().join(folder)]);
    let parts = classify_line(command, Quoting::Bash, scope, &mut folders, 0);
    Self { parts }
  }

  pub fn parts(&self) -> &[CommandPart] {
    &self.parts
  }

  /// The strictest part's category; Unknown for a command with no part.
  pub fn category(&self) -> CommandCategory {
    self.forcing().map_or(CommandCategory::Unknown, CommandPart::category)
  }

  /// The part that names the command: the strictest, the first of equals.
  pub fn forcing(&self) -> Option<&CommandPart> {
    self.parts.iter().rev().max_by_key(|part| part.category)
  }

  /// The parts that would ask by themselves: every part that is not Read-only.
  pub fn asking(&self) -> impl Iterator<Item = &CommandPart> {
    self
      .parts
      .iter()
      .filter(|part| part.category != CommandCategory::ReadOnly)
  }

  pub fn reads_secrets(&self) -> bool {
    self.parts.iter().any(CommandPart::reads_secrets)
  }

  pub fn installs(&self) -> bool {
    self.parts.iter().any(CommandPart::installs)
  }
}

fn classify_line(
  line: &str,
  quoting: Quoting,
  scope: &CommandScope,
  folders: &mut Folders,
  depth: usize,
) -> Vec<CommandPart> {
  words::split(line, quoting)
    .into_iter()
    .flat_map(|raw| classify_part(&raw, quoting, scope, folders, depth))
    .collect()
}

fn classify_part(
  raw: &RawPart,
  quoting: Quoting,
  scope: &CommandScope,
  folders: &mut Folders,
  depth: usize,
) -> Vec<CommandPart> {
  let words = raw.words.clone();
  // A variable set before a program (or for the commands after it) can change what any program does.
  if raw.unread || words.first().is_some_and(|word| is_assignment(word)) {
    return vec![CommandPart::unknown(words)];
  }
  let placing = Placing {
    scope,
    folders: folders.as_deref(),
  };
  let Some((first, args)) = words.split_first() else {
    // Only redirections.
    return vec![redirected(
      CommandPart::new(words, None, &Judged::of(Kind::ReadOnly)),
      raw,
      placing,
    )];
  };
  let program = program_key(first, quoting);
  if let Some(script) = program.as_deref().and_then(|program| shell_script(program, args)) {
    let Some((script, inner)) = script.filter(|_| depth < SHELL_DEPTH) else {
      return vec![CommandPart::unknown(words)];
    };
    // The script runs in a shell of its own: a `cd` in it changes nothing after it.
    let mut inner_folders = folders.clone();
    let mut parts = classify_line(&script, inner, scope, &mut inner_folders, depth + 1);
    if !raw.writes.is_empty() || !raw.reads.is_empty() {
      parts.push(redirected(
        CommandPart::new(words, None, &Judged::of(Kind::ReadOnly)),
        raw,
        placing,
      ));
    }
    return parts;
  }
  let Some(program) = program else {
    return vec![CommandPart::unknown(words)];
  };
  let judged = programs::judge(&program, args);
  let mut part = CommandPart::new(words.clone(), Some(program.clone()), &judged);
  for word in place::named(args)
    .into_iter()
    .chain(judged.reads.iter().map(String::as_str))
  {
    placing.read(&mut part, word);
  }
  for target in &judged.writes {
    placing.write(&mut part, target, judged.deletes);
  }
  for word in &judged.inside {
    placing.inside(&mut part, word);
  }
  let part = redirected(part, raw, placing);
  if matches!(program.as_str(), "cd" | "pushd" | "chdir" | "set-location") {
    place::change_directory(scope, folders, args);
  }
  vec![part]
}

/// `part` with the files its redirections read and write placed.
fn redirected(mut part: CommandPart, raw: &RawPart, placing: Placing<'_>) -> CommandPart {
  for read in &raw.reads {
    placing.read(&mut part, read);
  }
  for target in &raw.writes {
    placing.write(&mut part, target, false);
  }
  part
}

/// `NAME=value` sets a variable.
fn is_assignment(word: &str) -> bool {
  word.split_once('=').is_some_and(|(name, _)| {
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
      && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
  })
}

/// The program as the rules know it. A program written with a folder is known only as a shell
/// (`C:\…\pwsh.exe -Command …`), since any other could be a file the agent wrote; in a PowerShell
/// script names are matched without case and an alias is its cmdlet ([`powershell::cmdlet`]).
fn program_key(word: &str, quoting: Quoting) -> Option<String> {
  if word.contains(['/', '\\']) {
    let name = Path::new(word).file_stem()?.to_string_lossy().to_lowercase();
    return SHELLS.contains(&name.as_str()).then_some(name);
  }
  let name = program_name(word)?;
  Some(if quoting == Quoting::PowerShell {
    powershell::cmdlet(&name.to_lowercase()).to_owned()
  } else {
    name
  })
}

const SHELLS: [&str; 5] = ["bash", "sh", "zsh", "pwsh", "powershell"];

/// For a shell, the script it runs and its quoting, or `Some(None)` when it runs one Orchester cannot
/// read (a script file, an encoded command) or is started with an option that is not listed (a startup
/// file, another working folder); `None` for any other program.
fn shell_script(program: &str, args: &[String]) -> Option<Option<(String, Quoting)>> {
  match program {
    "bash" | "sh" | "zsh" => Some(posix_script(args)),
    "pwsh" | "powershell" => Some(powershell_script(args)),
    _ => None,
  }
}

/// `bash -c script` with listed options before it: clusters of option letters that change no file
/// read at start-up, `-o name`, `--login`, `--noprofile`, `--norc`.
fn posix_script(args: &[String]) -> Option<(String, Quoting)> {
  const LETTERS: &str = "abefhklmnptuvxBCHP";
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    match arg.as_str() {
      "--login" | "--noprofile" | "--norc" => {}
      "-o" | "+o" => {
        args.next()?;
      }
      cluster if cluster.starts_with('-') && cluster.len() > 1 => {
        let letters = &cluster[1..];
        if !letters.chars().all(|letter| letter == 'c' || LETTERS.contains(letter)) {
          return None;
        }
        if letters.contains('c') {
          return args.next().map(|script| (script.clone(), Quoting::Bash));
        }
      }
      _ => return None,
    }
  }
  None
}

/// `pwsh -Command script` with listed parameters before it (matched without case).
fn powershell_script(args: &[String]) -> Option<(String, Quoting)> {
  const SWITCHES: [&str; 10] = [
    "-noprofile",
    "-nop",
    "-noninteractive",
    "-noni",
    "-nologo",
    "-nol",
    "-mta",
    "-sta",
    "-noexit",
    "-noprofileloadtime",
  ];
  const VALUED: [&str; 4] = ["-executionpolicy", "-ep", "-outputformat", "-of"];
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    let lower = arg.to_lowercase();
    if lower.len() >= 2 && "-command".starts_with(&lower) {
      let script = args.map(String::as_str).collect::<Vec<_>>().join(" ");
      return (!script.is_empty()).then_some((script, Quoting::PowerShell));
    }
    if VALUED.contains(&lower.as_str()) {
      args.next()?;
    } else if !SWITCHES.contains(&lower.as_str()) {
      return None;
    }
  }
  None
}
