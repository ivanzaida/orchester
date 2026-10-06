//! Command categories (`docs/design/orchester-workspace.md`, "Command categories and approval policy",
//! board `U97QE`): a command an agent's CLI asks about is split into its simple commands, each is
//! classified by its program, subcommand, options and the paths it names, and the strictest part
//! names the command. Only what is understood is classified: an unknown program or subcommand, an
//! option that is not listed for its program, a variable set before a program (`PATH=…`,
//! `LD_PRELOAD=…`, `GIT_…=…`), a command in the background and anything not read to the end are
//! Unknown, so they ask. A variable the line sets to a plain literal is read where it is used
//! ([`variables`]), and a `cd` into the worktree is no part of its own: the parts after it are placed
//! from every folder it may leave them in.
//!
//! Run temp (a temporary file this run created) is not told apart yet: a temporary file is outside
//! the worktree like any other. Besides its category a part says whether it reads a credential
//! location ([`CommandScope`]) and whether it installs software, which auto-approval never approves
//! ([`crate::run::auto_verdict`]).

mod awk_program;
mod files;
mod gh;
mod git;
mod glob;
mod heredoc;
mod install;
mod interpreters;
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
mod scripts;
mod sed_script;
mod variables;
mod words;

use std::path::Path;

use serde::{Deserialize, Serialize};

pub use self::{
  part::{CommandPart, SimilarCommand},
  paths::CommandScope,
  reason::CommandReason,
};
use self::{
  place::{Line, Placing},
  programs::{Judged, Kind},
  variables::is_assignment,
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
    let mut line = Line::new(Some(vec![scope.worktree().join(folder)]));
    let mut parts = classify_line(command, Quoting::Bash, scope, &mut line, 0);
    // A `cd` into the worktree or a writable folder does nothing a person decides about; where the
    // parts after it run is followed all the same.
    if parts.iter().any(|part| !part.moves_inside()) {
      parts.retain(|part| !part.moves_inside());
    }
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
  text: &str,
  quoting: Quoting,
  scope: &CommandScope,
  line: &mut Line,
  depth: usize,
) -> Vec<CommandPart> {
  let mut parts = Vec::new();
  for raw in words::split(text, quoting) {
    parts.extend(classify_part(&raw, quoting, scope, line, depth));
    line.made(scope, &raw.writes);
  }
  parts
}

fn classify_part(
  raw: &RawPart,
  quoting: Quoting,
  scope: &CommandScope,
  line: &mut Line,
  depth: usize,
) -> Vec<CommandPart> {
  let words = raw.words.clone();
  // Its value was read into the parts after it.
  if raw.assigns {
    return Vec::new();
  }
  // A variable set before a program (or for the commands after it) can change what any program does,
  // and a command in the background goes on after the line.
  if raw.unread || raw.background || words.first().is_some_and(|word| is_assignment(word)) {
    return vec![CommandPart::unknown(words)];
  }
  let placing = line.placing(scope);
  let Some((first, args)) = words.split_first() else {
    // Only redirections.
    return vec![redirected(
      CommandPart::new(words, None, &Judged::of(Kind::ReadOnly)),
      raw,
      placing,
    )];
  };
  let program = program_key(first, quoting);
  let inline = program
    .as_deref()
    .filter(|program| interpreters::script(program, args).is_none());
  if let Some(script) = inline.and_then(|program| shell_script(program, args)) {
    let Some((script, inner)) = script.filter(|_| depth < SHELL_DEPTH) else {
      return vec![CommandPart::unknown(words)];
    };
    // The script runs in a shell of its own: a `cd` in it changes nothing after it, what it makes stays.
    let mut inner_line = line.clone();
    let mut parts = classify_line(&script, inner, scope, &mut inner_line, depth + 1);
    line.made = inner_line.made;
    let placing = line.placing(scope);
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
  part.edits &= !judged.sources.iter().any(|source| placing.moves_tree(source));
  // A script in the worktree, or git in a repository nested in it, changes no more than an edit could.
  // A script that is no regular file of the worktree is code from elsewhere, not read.
  if let Some(at) = judged.runs {
    part.edits = placing.plain_file(&args[at]);
    if !part.edits {
      part.at_least(CommandCategory::Unknown);
    }
  }
  if let Some(repository) = &judged.repository {
    part.edits = placing.nested_repository(repository);
  }
  // A program may read the folder it runs in (`ls`, `rg ''`, `find`), wherever an earlier `cd` left it.
  placing.read(&mut part, ".");
  let named: Vec<String> = args
    .iter()
    .enumerate()
    .filter(|(at, _)| judged.script != Some(*at))
    .map(|(_, arg)| arg.clone())
    .collect();
  for word in place::named(&named)
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
  let mut part = redirected(part, raw, placing);
  if program == "cd" {
    part.moves_inside = enters_plainly(args) && part.known_safe_read();
  }
  line.made(scope, &judged.writes);
  if matches!(program.as_str(), "cd" | "pushd" | "chdir" | "set-location") {
    place::change_directory(scope, &mut line.folders, args);
  }
  vec![part]
}

/// Whether `cd args` goes to the one folder its only argument names as written: no option, no `-`
/// (the previous folder), no `~` and no glob, which could lead elsewhere than the word reads.
fn enters_plainly(args: &[String]) -> bool {
  matches!(args, [target] if !target.starts_with(['-', '~']) && !target.contains(['*', '?', '[']))
}

/// `part` with the files its redirections read and write placed.
fn redirected(mut part: CommandPart, raw: &RawPart, placing: Placing<'_>) -> CommandPart {
  for read in &raw.reads {
    placing.read(&mut part, read);
  }
  for target in &raw.writes {
    placing.write(&mut part, target, false);
  }
  part.writes_output |= raw.writes.iter().any(|target| !place::is_null_device(target));
  part
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
