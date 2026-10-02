//! Command categories (`docs/design/orchester-workspace.md`, "Command categories and approval policy",
//! board `U97QE`): a command an agent's CLI asks about is split into its simple commands, each is
//! classified by its program, subcommand, options and the paths it writes, and the strictest part
//! names the command. What cannot be read to the end is Unknown; nothing is guessed.
//!
//! Run temp (a temporary file this run created) is not told apart yet: a temporary file is outside
//! the worktree like any other. Besides its category a part says whether it reads a credential
//! location ([`CommandScope`]) and whether it installs software, which auto-approval never approves
//! ([`crate::run::auto_verdict`]).

mod git;
mod packages;
mod paths;
mod programs;
mod words;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use self::paths::CommandScope;
use self::{
  paths::Place,
  programs::{Judged, Kind},
  words::{Quoting, RawPart},
};
use super::{CommandPrefix, known_safe::program_name};

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
  /// Not read to the end: a script, a variable, a substitution, a program Orchester does not know.
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

/// The program and subcommand a rule of Approve similar names, with the category of the part it was
/// made from (`k4cz1C`): `cargo add` in Network, `cp` in Outside write.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimilarCommand {
  program: String,
  subcommand: Option<String>,
  category: CommandCategory,
}

impl SimilarCommand {
  pub fn program(&self) -> &str {
    &self.program
  }

  pub fn subcommand(&self) -> Option<&str> {
    self.subcommand.as_deref()
  }

  pub const fn category(&self) -> CommandCategory {
    self.category
  }

  /// `cargo add`, or the program alone.
  pub fn words(&self) -> String {
    match &self.subcommand {
      Some(sub) => format!("{} {sub}", self.program),
      None => self.program.clone(),
    }
  }
}

/// One simple command of a command line, classified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandPart {
  /// The program and its arguments as written, without leading variable assignments.
  words: Vec<String>,
  /// The program as the rules know it; `None` when it is written with a folder or not read.
  program: Option<String>,
  /// The subcommand of a program that has them, and where it is among the words.
  subcommand: Option<(usize, String)>,
  category: CommandCategory,
  reads_secrets: bool,
  installs: bool,
}

impl CommandPart {
  pub fn words(&self) -> &[String] {
    &self.words
  }

  pub const fn category(&self) -> CommandCategory {
    self.category
  }

  /// It reads (or writes) a credential location of the home folder, or reads a credential store.
  pub const fn reads_secrets(&self) -> bool {
    self.reads_secrets
  }

  /// It installs software or packages.
  pub const fn installs(&self) -> bool {
    self.installs
  }

  /// What a rule of Approve similar made from this part names; `None` for an Unknown part.
  pub fn similar(&self) -> Option<SimilarCommand> {
    if self.category == CommandCategory::Unknown {
      return None;
    }
    Some(SimilarCommand {
      program: self.program.clone()?,
      subcommand: self.subcommand.as_ref().map(|(_, sub)| sub.clone()),
      category: self.category,
    })
  }

  /// Whether `rule` names this part: the same program and subcommand, classified the same.
  pub fn is_similar(&self, rule: &SimilarCommand) -> bool {
    self.similar().as_ref() == Some(rule)
  }

  /// The prefixes Always allow can add for this part, widest first: the program and its subcommand
  /// as written, when the subcommand follows the program, and the whole part.
  pub fn prefixes(&self) -> Vec<CommandPrefix> {
    let mut prefixes = Vec::new();
    let head = match &self.subcommand {
      Some((1, _)) => Some(&self.words[..2]),
      Some(_) => None,
      None => Some(&self.words[..1]),
    };
    if let Some(head) = head.and_then(|head| CommandPrefix::new(&head.join(" ")).ok()) {
      prefixes.push(head);
    }
    if let Ok(whole) = CommandPrefix::new(&self.words.join(" "))
      && !prefixes.contains(&whole)
    {
      prefixes.push(whole);
    }
    prefixes
  }

  /// Whether a prefix of this part's program and subcommand would also allow a History / destructive
  /// form of them (`git reset` also allows `git reset --hard`, `rm` also `rm -rf ~`).
  pub fn has_destructive_forms(&self) -> bool {
    let Some(program) = self.program.as_deref() else {
      return true;
    };
    match (&self.subcommand, program) {
      (Some((_, sub)), "git") => git::WITH_DESTRUCTIVE_FORMS.contains(&sub.as_str()),
      (_, program) => programs::DESTRUCTIVE_PROGRAMS.contains(&program),
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
    let mut cwd = Some(scope.worktree().to_path_buf());
    let parts = classify_line(command, Quoting::Bash, scope, &mut cwd, 0);
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
  cwd: &mut Option<PathBuf>,
  depth: usize,
) -> Vec<CommandPart> {
  words::split(line, quoting)
    .into_iter()
    .flat_map(|raw| classify_part(&raw, quoting, scope, cwd, depth))
    .collect()
}

fn classify_part(
  raw: &RawPart,
  quoting: Quoting,
  scope: &CommandScope,
  cwd: &mut Option<PathBuf>,
  depth: usize,
) -> Vec<CommandPart> {
  let words: Vec<String> = raw
    .words
    .iter()
    .skip_while(|word| is_assignment(word))
    .cloned()
    .collect();
  if raw.unread {
    return vec![unknown(words)];
  }
  let Some((first, args)) = words.split_first() else {
    // Only assignments, perhaps with redirections.
    return vec![placed(
      CommandPart::new(words, None, &Judged::of(Kind::ReadOnly)),
      raw,
      scope,
      cwd.as_deref(),
    )];
  };
  let program = program_key(first, quoting);
  if let Some(script) = program.as_deref().and_then(|program| shell_script(program, args)) {
    return match script {
      Some((script, inner)) if depth < SHELL_DEPTH => classify_line(&script, inner, scope, cwd, depth + 1),
      _ => vec![unknown(words)],
    };
  }
  let Some(program) = program else {
    return vec![unknown(words)];
  };
  let judged = programs::judge(&program, args);
  let mut part = CommandPart::new(words.clone(), Some(program.clone()), &judged);
  part.subcommand = subcommand_of(&program, args);
  let part = placed(part, raw, scope, cwd.as_deref());
  let part = with_writes(part, &judged, scope, cwd.as_deref());
  if is_change_directory(&program) {
    *cwd = change_directory(scope, cwd.as_deref(), args);
  }
  vec![part]
}

impl CommandPart {
  fn new(words: Vec<String>, program: Option<String>, judged: &Judged) -> Self {
    let category = match judged.kind {
      Kind::ReadOnly => CommandCategory::ReadOnly,
      Kind::Worktree | Kind::Writes { .. } => CommandCategory::Worktree,
      Kind::Network => CommandCategory::Network,
      Kind::Destructive => CommandCategory::HistoryDestructive,
      Kind::Unknown => CommandCategory::Unknown,
    };
    Self {
      words,
      program,
      subcommand: None,
      category,
      reads_secrets: judged.secret_store,
      installs: judged.installs,
    }
  }

  fn at_least(&mut self, category: CommandCategory) {
    self.category = self.category.max(category);
  }
}

fn unknown(words: Vec<String>) -> CommandPart {
  CommandPart::new(words, None, &Judged::of(Kind::Unknown))
}

/// The part with what its redirections write and every word that names a path placed: a write outside
/// the worktree is Outside write, and a credential location read or written is noted.
fn placed(mut part: CommandPart, raw: &RawPart, scope: &CommandScope, cwd: Option<&Path>) -> CommandPart {
  let named = part
    .words
    .iter()
    .skip(1)
    .map(|word| word.split_once('=').map_or(word.as_str(), |(_, value)| value))
    .chain(raw.reads.iter().map(String::as_str))
    .filter(|word| !word.is_empty());
  let mut reads_secrets = false;
  for word in named {
    if let Some(path) = cwd.and_then(|cwd| scope.path(cwd, word)) {
      reads_secrets |= scope.place(&path) == Place::Secret;
    }
  }
  part.reads_secrets |= reads_secrets;
  for target in raw.writes.iter().filter(|target| !is_null_device(target)) {
    place_write(&mut part, target, false, scope, cwd);
  }
  part
}

/// The part with the files its program writes placed.
fn with_writes(mut part: CommandPart, judged: &Judged, scope: &CommandScope, cwd: Option<&Path>) -> CommandPart {
  let delete = matches!(judged.kind, Kind::Writes { delete: true });
  for target in &judged.writes {
    place_write(&mut part, target, delete, scope, cwd);
  }
  part
}

fn place_write(part: &mut CommandPart, target: &str, delete: bool, scope: &CommandScope, cwd: Option<&Path>) {
  let Some(path) = cwd.and_then(|cwd| scope.path(cwd, target)) else {
    part.at_least(CommandCategory::Unknown);
    return;
  };
  let category = match scope.place(&path) {
    Place::Granted => CommandCategory::Worktree,
    Place::Temp => CommandCategory::OutsideWrite,
    Place::Outside | Place::Secret if delete => CommandCategory::HistoryDestructive,
    Place::Outside | Place::Secret => CommandCategory::OutsideWrite,
  };
  part.reads_secrets |= scope.place(&path) == Place::Secret;
  part.at_least(category);
}

fn is_null_device(target: &str) -> bool {
  target == "/dev/null" || target.eq_ignore_ascii_case("nul")
}

/// `NAME=value` before a program sets a variable for it; the value is a plain word.
fn is_assignment(word: &str) -> bool {
  word.split_once('=').is_some_and(|(name, _)| {
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
      && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
  })
}

/// The program as the rules know it. A program written with a folder is known only as a shell
/// (`C:\…\pwsh.exe -Command …`), since any other could be a file the agent wrote; PowerShell's
/// commands are matched without case.
fn program_key(word: &str, quoting: Quoting) -> Option<String> {
  if word.contains(['/', '\\']) {
    let name = Path::new(word).file_stem()?.to_string_lossy().to_lowercase();
    return SHELLS.contains(&name.as_str()).then_some(name);
  }
  let name = program_name(word)?;
  Some(if quoting == Quoting::PowerShell {
    name.to_lowercase()
  } else {
    name
  })
}

const SHELLS: [&str; 5] = ["bash", "sh", "zsh", "pwsh", "powershell"];

/// For a shell, the script it runs and its quoting, or `Some(None)` when it runs one Orchester cannot
/// read (a script file, an encoded command); `None` for any other program.
fn shell_script(program: &str, args: &[String]) -> Option<Option<(String, Quoting)>> {
  match program {
    "bash" | "sh" | "zsh" => {
      let at = args
        .iter()
        .position(|arg| arg.starts_with('-') && !arg.starts_with("--") && arg.contains('c'))?;
      Some(args.get(at + 1).map(|script| (script.clone(), Quoting::Bash)))
    }
    "pwsh" | "powershell" => {
      let at = args.iter().position(|arg| {
        let lower = arg.to_lowercase();
        lower.len() >= 2 && "-command".starts_with(&lower)
      });
      Some(at.and_then(|at| {
        let script = args[at + 1..].join(" ");
        (!script.is_empty()).then_some((script, Quoting::PowerShell))
      }))
    }
    _ => None,
  }
}

fn subcommand_of(program: &str, args: &[String]) -> Option<(usize, String)> {
  if !packages::has_subcommands(program) {
    return None;
  }
  let (at, sub) = if program == "git" {
    git::subcommand(args)?
  } else {
    args
      .iter()
      .enumerate()
      .find(|(_, arg)| !arg.starts_with('-'))
      .map(|(at, arg)| (at, arg.as_str()))?
  };
  // Positions count the program as word 0.
  Some((at + 1, sub.to_owned()))
}

fn is_change_directory(program: &str) -> bool {
  matches!(program, "cd" | "pushd" | "chdir" | "set-location" | "sl")
}

/// The folder later parts run in after `cd args`; `None` when it cannot be told.
fn change_directory(scope: &CommandScope, cwd: Option<&Path>, args: &[String]) -> Option<PathBuf> {
  let target = programs::operands(args)
    .into_iter()
    .next()
    .unwrap_or_else(|| "~".to_owned());
  scope.path(cwd?, &target)
}
