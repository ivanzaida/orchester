//! What one simple command does by its program, subcommand and options (board `U97QE`), before the
//! paths it names are placed: the tables of programs that only read, that build or test in the
//! worktree, and that read a credential store, with the modules for files, the network, git, `gh`,
//! PowerShell and package managers. Only what is understood is classified: a program none of them
//! knows, or an option of a known one that is not listed, is Unknown.
//!
//! The one place that is not an allow-list: the reading programs of [`READS`] (and PowerShell's reading
//! cmdlets in [`powershell`]) are taken with any option but those known to write a file, start a
//! program or set a variable, which [`plain_read`] refuses. Every path they name is still placed, so a
//! credential location asks.

use super::{files, gh, git, network, packages, powershell};
use crate::permission::known_safe;

/// What a command is by its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  ReadOnly,
  /// Builds, tests or otherwise changes the worktree without naming the files.
  Worktree,
  Network,
  /// Rewrites or discards history.
  Destructive,
  /// Writes the files it names (or redirects to).
  Writes,
  Unknown,
}

/// A command's kind with the words that name files it reads and writes, and what else it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Judged {
  pub(super) kind: Kind,
  /// Words naming files the command writes or deletes.
  pub(super) writes: Vec<String>,
  /// It removes what it writes.
  pub(super) deletes: bool,
  /// Words naming files it reads that are not plain arguments (`curl -T file`, `-d @file`).
  pub(super) reads: Vec<String>,
  /// Words that must lie in the worktree or a writable folder, else the command is Unknown: a build
  /// pointed elsewhere, the target of a link.
  pub(super) inside: Vec<String>,
  /// It installs software or packages.
  pub(super) installs: bool,
  /// It reads a credential store (a keychain, Windows credentials, a password manager, a token).
  pub(super) secret_store: bool,
  /// For an install, where the software comes from and the first package named.
  pub(super) source: Option<(String, Option<String>)>,
  /// The subcommand, and where it starts among the arguments, for a program that has them.
  pub(super) subcommand: Option<(usize, String)>,
  /// A later command of the same program and subcommand can be alike; not when the arguments are a
  /// command for another machine (`ssh host …`).
  pub(super) alike: bool,
  /// A prefix of the program and subcommand allows only commands like this one, whatever arguments
  /// follow: builds and tests that run the worktree's own code anyway, installs, and the git
  /// subcommands that cannot reach outside the repository.
  pub(super) prefix_stays: bool,
}

impl Judged {
  pub(super) const fn of(kind: Kind) -> Self {
    Self {
      kind,
      writes: Vec::new(),
      deletes: false,
      reads: Vec::new(),
      inside: Vec::new(),
      installs: false,
      secret_store: false,
      source: None,
      subcommand: None,
      alike: true,
      prefix_stays: false,
    }
  }

  pub(super) const fn unknown() -> Self {
    Self::of(Kind::Unknown)
  }

  /// A command of `kind` that writes `paths`.
  pub(super) fn writing(kind: Kind, paths: Vec<String>) -> Self {
    Self {
      writes: paths,
      ..Self::of(kind)
    }
  }

  pub(super) fn reading(mut self, paths: Vec<String>) -> Self {
    self.reads.extend(paths);
    self
  }

  pub(super) const fn deleting(mut self) -> Self {
    self.deletes = true;
    self
  }

  /// Every argument, and the value of every `--option=value`, must lie inside.
  pub(super) fn inside_all(mut self, args: &[String]) -> Self {
    self
      .inside
      .extend(args.iter().filter_map(|arg| match arg.strip_prefix('-') {
        Some(option) => option.split_once('=').map(|(_, value)| value.to_owned()),
        None => Some(arg.clone()),
      }));
    self
  }

  pub(super) const fn installing(mut self) -> Self {
    self.installs = true;
    self
  }

  /// An install from `registry` of `package`.
  pub(super) fn installed_from(mut self, registry: &str, package: Option<String>) -> Self {
    self.source = Some((registry.to_owned(), package));
    self
  }

  pub(super) const fn secret(mut self) -> Self {
    self.secret_store = true;
    self
  }

  pub(super) const fn not_alike(mut self) -> Self {
    self.alike = false;
    self
  }

  pub(super) const fn prefix_stays(mut self) -> Self {
    self.prefix_stays = true;
    self
  }

  /// The subcommand `name`, starting at argument `at`.
  pub(super) fn subcommand(mut self, at: usize, name: String) -> Self {
    self.subcommand = Some((at, name));
    self
  }
}

/// Programs that read or print and change nothing, other than those [`known_safe`] rules by their
/// options. Their options that write a file, start a program or set a shell variable are refused in
/// [`plain_read`].
const READS: [&str; 30] = [
  "grep",
  "egrep",
  "fgrep",
  "tree",
  "du",
  "df",
  "diff",
  "cmp",
  "uniq",
  "cut",
  "tr",
  "jq",
  "date",
  "basename",
  "dirname",
  "realpath",
  "readlink",
  "true",
  "false",
  "test",
  "[",
  "sleep",
  "nl",
  "column",
  "md5sum",
  "sha256sum",
  "sha1sum",
  "cd",
  "whereis",
  "printf",
];

/// Build, test and run tools that work in the worktree. Every path they are given must lie inside.
const BUILDS: [&str; 18] = [
  "make",
  "cmake",
  "ninja",
  "just",
  "mvn",
  "gradle",
  "gradlew",
  "pytest",
  "tsc",
  "eslint",
  "rustfmt",
  "clang-format",
  "prettier",
  "black",
  "ruff",
  "mypy",
  "jest",
  "vitest",
];

/// The build tools a prefix of which allows nothing more than the worktree's own build or tests, which
/// run any code the worktree holds anyway. Formatters and `cmake` (`cmake -E rm`) are not among them:
/// their arguments name the files they change.
const RUNS_WORKTREE_CODE: [&str; 9] = [
  "make", "ninja", "just", "mvn", "gradle", "gradlew", "pytest", "jest", "vitest",
];

/// Programs that read a credential store.
const SECRET_STORES: [&str; 6] = [
  "security",
  "cmdkey",
  "vaultcmd",
  "secret-tool",
  "pass",
  "get-storedcredential",
];

/// `program` (its name as [`super::program_key`] gives it) with `args`.
pub(super) fn judge(program: &str, args: &[String]) -> Judged {
  if SECRET_STORES.contains(&program) {
    return Judged::unknown().secret();
  }
  if let Some(judged) = packages::judge(program, args)
    .or_else(|| files::judge(program, args))
    .or_else(|| network::judge(program, args))
    .or_else(|| powershell::judge(program, args))
  {
    return judged;
  }
  match program {
    "git" => git::judge(args),
    "gh" => gh::judge(args),
    // `env` alone prints every variable, tokens included.
    "env" if args.is_empty() => Judged::of(Kind::ReadOnly).secret(),
    program if known_safe::PROGRAMS.contains(&program) => known_read(program, args),
    program if READS.contains(&program) => plain_read(program, args),
    program if BUILDS.contains(&program) => {
      let judged = Judged::of(Kind::Worktree).inside_all(args);
      if RUNS_WORKTREE_CODE.contains(&program) {
        judged.prefix_stays()
      } else {
        judged
      }
    }
    _ => Judged::unknown(),
  }
}

/// A program [`known_safe`] rules: Read-only when every option is one it lists, else Unknown. The words
/// that may name a file, attached option values included, are placed.
fn known_read(program: &str, args: &[String]) -> Judged {
  let argv: Vec<String> = std::iter::once(program.to_owned())
    .chain(args.iter().cloned())
    .collect();
  match known_safe::readable_arguments(&argv) {
    Some(words) => Judged::of(Kind::ReadOnly).reading(words.into_iter().map(str::to_owned).collect()),
    None => Judged::unknown(),
  }
}

/// A program of [`READS`], Unknown with an option that writes a file (`tree -o`, `uniq in out`), sets
/// the system date or a shell variable (`date -s`, `printf -v`), or a `jq` program that reads the
/// environment.
fn plain_read(program: &str, args: &[String]) -> Judged {
  let refused = match program {
    "tree" => has_option(args, &["-o", "-R", "--output"]),
    "uniq" => operands(args).len() > 1,
    "date" => has_option(args, &["-s", "--set"]) || operands(args).iter().any(|operand| !operand.starts_with('+')),
    "printf" => has_option(args, &["-v"]),
    "jq" => args
      .iter()
      .any(|arg| arg.contains("env") || arg.contains("ENV") || arg.contains("input_filename")),
    _ => false,
  };
  if refused {
    Judged::unknown()
  } else {
    Judged::of(Kind::ReadOnly)
  }
}

/// The arguments that are not options. `--` ends the options.
pub(super) fn operands(args: &[String]) -> Vec<String> {
  let mut operands = Vec::new();
  let mut options = true;
  for arg in args {
    if options && arg == "--" {
      options = false;
    } else if !(options && arg.starts_with('-') && arg.len() > 1) {
      operands.push(arg.clone());
    }
  }
  operands
}

/// Whether one of `args` is one of `options`, or a short option cluster holding one of the short
/// ones (`-fd` holds `-f`).
pub(super) fn has_option(args: &[String], options: &[&str]) -> bool {
  args.iter().any(|arg| {
    options.iter().any(|option| {
      arg == option
        || arg.strip_prefix(option).is_some_and(|rest| rest.starts_with('='))
        || (option.len() == 2
          && !option.starts_with("--")
          && arg.starts_with('-')
          && !arg.starts_with("--")
          && arg[1..].contains(&option[1..]))
    })
  })
}
