//! One simple command, classified, and what Approve similar and Always allow may make of it. Both only
//! cover what is no more dangerous than what a person approved: Approve similar never covers a part
//! that rewrites history, deletes outside the worktree, reads a credential or was not read, nor one
//! that reaches outside the worktree anywhere the approved part did not; Always allow offers a prefix
//! only where every command it allows stays like the approved one ([`CommandPart::has_wider_forms`]).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
  CommandCategory,
  build_output::BuildOutput,
  install::InstallSource,
  programs::{Judged, Kind},
};
use crate::permission::CommandPrefix;

/// The program and subcommand a rule of Approve similar names, with the category of the part it was
/// made from (`k4cz1C`): `cargo add` in Network, `cp` in Outside write, and the folders outside the
/// worktree that part reached, where a later part may also reach.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SimilarCommand {
  program: String,
  subcommand: Option<String>,
  category: CommandCategory,
  /// A rule stored before this was kept has none, so it covers nothing outside the worktree.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  outside: Vec<PathBuf>,
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

  /// The folders outside the worktree the rule also covers.
  pub fn outside(&self) -> &[PathBuf] {
    &self.outside
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
  /// The program and its arguments as written.
  pub(super) words: Vec<String>,
  /// The program as the rules know it; `None` when it is written with a folder or not read.
  pub(super) program: Option<String>,
  /// The subcommand of a program that has them (`status`, `pr view`), and the word it starts at.
  pub(super) subcommand: Option<(usize, String)>,
  pub(super) category: CommandCategory,
  pub(super) reads_secrets: bool,
  pub(super) installs: bool,
  /// The first word that names a credential location, as written.
  pub(super) secret_word: Option<String>,
  /// The first file written outside the worktree, and the first deleted outside the temporary
  /// folder, as written.
  pub(super) outside_write: Option<String>,
  pub(super) outside_delete: Option<String>,
  /// Where an install comes from and the first package named ([`Judged::source`]).
  pub(super) source: Option<InstallSource>,
  /// The folders outside the worktree and its writable folders that the part names or writes.
  pub(super) outside: Vec<PathBuf>,
  /// A later part of the same program and subcommand can be alike ([`Judged::alike`]).
  pub(super) alike: bool,
  /// A prefix of its program and subcommand allows only parts like it ([`Judged::prefix_stays`]).
  pub(super) prefix_stays: bool,
  /// Its words are a read of the known-safe tables ([`Judged::known_safe`]).
  pub(super) known_safe: bool,
  /// It redirects its output into a file (`> out.txt`); not the null device.
  pub(super) writes_output: bool,
  /// A `cd` to one folder named plainly, in the worktree or a writable folder.
  pub(super) moves_inside: bool,
  /// It runs a script file of the worktree ([`super::interpreters`]).
  pub(super) runs_script: bool,
  /// It runs inside the inline script of a shell written with a folder, which is not the bare shell a
  /// rule names.
  pub(super) through_path: bool,
  /// Its program only edits the files it names ([`Judged::edits`]).
  pub(super) edits: bool,
  /// A Cargo build's target directory, where it writes its output ([`super::build_output`]).
  pub(super) build_output: Option<BuildOutput>,
  /// A Network part that only asks ([`Judged::looks_up`]).
  pub(super) looks_up: bool,
  /// It may write a tool's cache or a lock file ([`Judged::updates_cache`]).
  pub(super) updates_cache: bool,
}

impl CommandPart {
  pub(super) fn new(words: Vec<String>, program: Option<String>, judged: &Judged) -> Self {
    let category = match judged.kind {
      Kind::ReadOnly => CommandCategory::ReadOnly,
      Kind::Worktree | Kind::Writes => CommandCategory::Worktree,
      Kind::Network => CommandCategory::Network,
      Kind::Destructive => CommandCategory::HistoryDestructive,
      Kind::Unknown => CommandCategory::Unknown,
    };
    Self {
      words,
      subcommand: judged.subcommand.as_ref().map(|(at, sub)| (at + 1, sub.clone())),
      program,
      category,
      reads_secrets: judged.secret_store,
      installs: judged.installs,
      secret_word: None,
      outside_write: None,
      outside_delete: None,
      source: judged.source.clone(),
      outside: Vec::new(),
      alike: judged.alike,
      prefix_stays: judged.prefix_stays,
      known_safe: judged.known_safe,
      writes_output: false,
      moves_inside: false,
      runs_script: false,
      through_path: false,
      edits: judged.edits,
      build_output: None,
      looks_up: judged.looks_up,
      updates_cache: judged.updates_cache,
    }
  }

  pub(super) fn unknown(words: Vec<String>) -> Self {
    Self::new(words, None, &Judged::unknown())
  }

  pub(super) fn at_least(&mut self, category: CommandCategory) {
    self.category = self.category.max(category);
  }

  pub(super) fn reaches(&mut self, folder: PathBuf) {
    if !self.outside.contains(&folder) {
      self.outside.push(folder);
    }
  }

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

  /// Whether it names, reads or writes anything outside the worktree and its writable folders.
  pub const fn reaches_outside(&self) -> bool {
    !self.outside.is_empty()
  }

  /// The program as the rules know it; `None` when it is written with a folder or was not read.
  pub fn program(&self) -> Option<&str> {
    self.program.as_deref()
  }

  /// The subcommand (`push`, `pr view`), when the program has them.
  pub fn subcommand(&self) -> Option<&str> {
    self.subcommand.as_ref().map(|(_, sub)| sub.as_str())
  }

  /// The words after the subcommand; all words after the program for one without a subcommand.
  pub fn after_subcommand(&self) -> &[String] {
    let start = match &self.subcommand {
      Some((at, sub)) => at + sub.split(' ').count(),
      None => 1,
    };
    self.words.get(start..).unwrap_or_default()
  }

  /// Whether it writes a file outside the worktree, or deletes one outside the temporary folder.
  pub const fn writes_outside(&self) -> bool {
    self.outside_write.is_some() || self.outside_delete.is_some()
  }

  /// The target directory this part, a Cargo build, writes its output into, and nothing else Orchester can tell of
  /// ([`super::build_output`]); `None` for any other part, and for one that also redirects into a file, reaches
  /// outside the worktree and its writable folders, reads a credential, installs or was not read to the end.
  pub fn build_output(&self) -> Option<&BuildOutput> {
    self.build_output.as_ref().filter(|_| {
      self.category == CommandCategory::Worktree
        && !self.writes_output
        && !self.reads_secrets
        && !self.installs
        && !self.runs_script
        && self.outside.is_empty()
    })
  }

  /// Whether it writes nothing at all, a tool's own cache outside the worktree included (the person's decision of
  /// 2026-10-23): a read, or a lookup over the network ([`Judged::looks_up`]), that refreshes no cache or lock file,
  /// redirects into no file and writes nothing outside the worktree.
  pub const fn writes_nothing(&self) -> bool {
    let reads = match self.category {
      CommandCategory::ReadOnly => true,
      CommandCategory::Network => self.looks_up,
      _ => false,
    };
    reads && !self.updates_cache && !self.writes_output && !self.installs && !self.runs_script && !self.writes_outside()
  }

  /// Whether it runs a script file of the worktree, whose content Orchester does not read.
  pub const fn runs_script(&self) -> bool {
    self.runs_script
  }

  /// Whether it runs in a shell written with a folder (`C:\Program Files\PowerShell\7\pwsh.exe -Command …`):
  /// no rule for a bare program name runs it.
  pub const fn through_path(&self) -> bool {
    self.through_path
  }

  /// Whether it redirects its output into a file.
  pub const fn writes_output(&self) -> bool {
    self.writes_output
  }

  /// Whether it runs without a person on its own (the user's decision, `docs/plan.md` C4): a read of the
  /// known-safe tables, without a redirection into a file, of nothing outside the worktree and its
  /// writable folders and of no credential.
  pub fn known_safe_read(&self) -> bool {
    self.known_safe
      && self.category == CommandCategory::ReadOnly
      && !self.reads_secrets
      && !self.installs
      && !self.writes_output
      && self.outside.is_empty()
  }

  /// Whether it only writes files in the worktree or its writable folders, as an edit does: a program that
  /// edits the files it names (`sed -i`, `tee`, `mkdir`, `touch`, `cp`, `mv`, `rm`) or a known-safe read
  /// whose output is redirected there (`cat > f <<'EOF'`, `echo x >> f`), every write placed where an
  /// edit may write ([`super::CommandScope`]: no `.git`, no `.orchester/`, not the folder itself
  /// removed), and nothing named, read or written outside, no credential.
  pub fn edits_worktree(&self) -> bool {
    (self.edits || self.known_safe)
      && self.category == CommandCategory::Worktree
      && !self.reads_secrets
      && !self.installs
      && self.outside.is_empty()
  }

  /// Whether it is a `cd` to one folder named plainly in the worktree or a writable folder, which
  /// changes nothing a person decides about.
  pub(super) const fn moves_inside(&self) -> bool {
    self.moves_inside
  }

  /// Whether a person must answer this part each time: it was not read to the end, rewrites history or
  /// deletes outside the worktree, or reads a credential. No wider answer covers it.
  pub const fn always_asks(&self) -> bool {
    matches!(
      self.category,
      CommandCategory::Unknown | CommandCategory::HistoryDestructive
    ) || self.reads_secrets
  }

  /// What a rule of Approve similar made from this part names; `None` for a part that always asks
  /// ([`Self::always_asks`]) or whose later parts cannot be told alike (`ssh host …`).
  pub fn similar(&self) -> Option<SimilarCommand> {
    if self.always_asks() || !self.alike {
      return None;
    }
    Some(SimilarCommand {
      program: self.program.clone()?,
      subcommand: self.subcommand.as_ref().map(|(_, sub)| sub.clone()),
      category: self.category,
      outside: self.outside.clone(),
    })
  }

  /// Whether `rule` names this part: the same program, subcommand and category, reaching outside the
  /// worktree only into folders the rule's part reached.
  pub fn is_similar(&self, rule: &SimilarCommand) -> bool {
    self.similar().is_some_and(|this| {
      this.program == rule.program
        && this.subcommand == rule.subcommand
        && this.category == rule.category
        && this.outside.iter().all(|folder| rule.outside.contains(folder))
    })
  }

  /// The prefixes Always allow can add for this part, widest first: the program and its subcommand
  /// as written, when the subcommand follows the program, and the whole part.
  pub fn prefixes(&self) -> Vec<CommandPrefix> {
    let mut prefixes = Vec::new();
    let head = match &self.subcommand {
      Some((1, sub)) => self.words.get(..1 + sub.split(' ').count()),
      Some(_) => None,
      None => self.words.get(..1),
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

  /// Whether a prefix of this part's program and subcommand would also allow a more dangerous form of
  /// them: a History / destructive one (`git reset --hard`, `rm -rf ~`), one that starts another
  /// program or writes outside (`git fetch --upload-pack=…`, `curl -o ~/x`), or one that reads a
  /// credential (`cat ~/.ssh/id_rsa`). Only builds, tests and installs, which run code of the
  /// worktree or its packages whatever their arguments, and the git subcommands that stay in the
  /// repository have none.
  pub const fn has_wider_forms(&self) -> bool {
    !self.prefix_stays
  }
}
