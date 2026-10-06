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

use super::{files, gh, git, install::InstallSource, interpreters, network, packages, powershell, processes, scripts};
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
  pub(super) source: Option<InstallSource>,
  /// The subcommand, and where it starts among the arguments, for a program that has them.
  pub(super) subcommand: Option<(usize, String)>,
  /// A later command of the same program and subcommand can be alike; not when the arguments are a
  /// command for another machine (`ssh host …`).
  pub(super) alike: bool,
  /// A prefix of the program and subcommand allows only commands like this one, whatever arguments
  /// follow: builds and tests that run the worktree's own code anyway, installs, and the git
  /// subcommands that cannot reach outside the repository.
  pub(super) prefix_stays: bool,
  /// A read the known-safe tables judge by their options ([`known_safe`], [`READS`], print-only `sed` and
  /// `awk`): it starts no other program and writes nothing.
  pub(super) known_safe: bool,
  /// The argument that is a script (`sed`'s, `awk`'s program) rather than a path.
  pub(super) script: Option<usize>,
  /// It only creates, changes, copies, moves or removes the files it names, as an edit does (`sed -i`,
  /// `tee`, `mkdir`, `touch`, `cp`, `mv`, `rm`), and starts no other program.
  pub(super) edits: bool,
  /// What `cp` and `mv` copy or move.
  pub(super) sources: Vec<String>,
  /// The argument that is a script file an interpreter runs ([`interpreters`]).
  pub(super) runs: Option<usize>,
  /// It runs inline code (`python -c`, `node -e`), whose argument is [`Self::script`].
  pub(super) inline: bool,
  /// The inline code is its standard input (`python -`), which only a here-document makes the agent's
  /// own code as written.
  pub(super) stdin_code: bool,
  /// Its arguments name no file (`tasklist /FI …`): none is placed.
  pub(super) no_paths: bool,
  /// The repository `git -C <folder>` works in.
  pub(super) repository: Option<String>,
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
      known_safe: false,
      script: None,
      edits: false,
      sources: Vec::new(),
      runs: None,
      inline: false,
      stdin_code: false,
      no_paths: false,
      repository: None,
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

  /// An install, from `source`.
  pub(super) fn installed_from(mut self, source: InstallSource) -> Self {
    self.source = Some(source);
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

  /// A Read-only command of the known-safe tables.
  pub(super) const fn known_safe(mut self) -> Self {
    self.known_safe = true;
    self
  }

  /// A command that edits the files it names, as [`Self::edits`] says.
  pub(super) const fn editing(mut self) -> Self {
    self.edits = true;
    self
  }

  /// A command whose arguments name no file.
  pub(super) const fn naming_no_file(mut self) -> Self {
    self.no_paths = true;
    self
  }

  /// A copy or move of `sources`.
  pub(super) fn moving(mut self, sources: Vec<String>) -> Self {
    self.sources = sources;
    self
  }

  /// Argument `at` is a script, not a path.
  pub(super) const fn with_script(mut self, at: Option<usize>) -> Self {
    self.script = at;
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
const READS: [&str; 31] = [
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
  "sort",
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
  if interpreters::reads_code(program, args) {
    return Judged {
      inline: true,
      stdin_code: true,
      ..Judged::of(Kind::Worktree)
    };
  }
  if let Some(at) = interpreters::inline(program, args) {
    return Judged {
      inline: true,
      script: Some(at),
      ..Judged::of(Kind::Worktree)
    };
  }
  if let Some(at) = interpreters::script(program, args) {
    return Judged {
      runs: Some(at),
      ..Judged::of(Kind::Worktree)
    };
  }
  if let Some(judged) = packages::judge(program, args)
    .or_else(|| files::judge(program, args))
    .or_else(|| scripts::judge(program, args))
    .or_else(|| processes::judge(program, args))
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
    Some(words) => Judged::of(Kind::ReadOnly)
      .known_safe()
      .reading(words.into_iter().map(str::to_owned).collect()),
    None => Judged::unknown(),
  }
}

/// A program of [`READS`], Unknown with an option that writes a file (`tree -o`, `uniq in out`, `sort -o`),
/// sets the system date or a shell variable (`date -s`, `printf -v`), starts a program (`sort
/// --compress-program`), reads the files a file names (`sort --files0-from`), follows links (`grep -R`),
/// or a `jq` program that reads the environment or a module.
fn plain_read(program: &str, args: &[String]) -> Judged {
  let refused = match program {
    "tree" => has_option(args, &["-o", "-R", "--output"]),
    "uniq" => operands(args).len() > 1,
    "date" => has_option(args, &["-s", "--set"]) || operands(args).iter().any(|operand| !operand.starts_with('+')),
    "printf" => has_option(args, &["-v"]),
    // `-R` follows links, also out of the folders it is given.
    "grep" | "egrep" | "fgrep" => has_option(args, &["-R"]) || abbreviates(args, &["dereference-recursive"]),
    "sort" => has_option(args, &["-o", "-T"]) || abbreviates(args, &SORT_REFUSED),
    // A module (`include`, `import`, `-L`) is read from folders the arguments do not name.
    "jq" => {
      has_option(args, &["-L"])
        || args.iter().any(|arg| {
          ["env", "ENV", "input_filename", "include", "import"]
            .iter()
            .any(|refused| arg.contains(refused))
        })
    }
    _ => false,
  };
  if refused {
    Judged::unknown()
  } else {
    Judged::of(Kind::ReadOnly).known_safe()
  }
}

/// `sort`'s long options that write a file, start a program or read the files a file names.
const SORT_REFUSED: [&str; 4] = ["output", "compress-program", "files0-from", "temporary-directory"];

/// Whether one of `args` is one of the long options `names` (without `--`), or an abbreviation of
/// one, which `getopt_long` takes for it.
fn abbreviates(args: &[String], names: &[&str]) -> bool {
  args.iter().take_while(|arg| *arg != "--").any(|arg| {
    arg
      .strip_prefix("--")
      .map(|long| long.split_once('=').map_or(long, |(name, _)| name))
      .is_some_and(|given| !given.is_empty() && names.iter().any(|name| name.starts_with(given)))
  })
}

/// The arguments that are not options. `--` ends the options.
pub(super) fn operands(args: &[String]) -> Vec<String> {
  operand_positions(args).into_iter().map(|at| args[at].clone()).collect()
}

/// Where the arguments that are not options are among `args`.
pub(super) fn operand_positions(args: &[String]) -> Vec<usize> {
  let mut positions = Vec::new();
  let mut options = true;
  for (at, arg) in args.iter().enumerate() {
    if options && arg == "--" {
      options = false;
    } else if !(options && arg.starts_with('-') && arg.len() > 1) {
      positions.push(at);
    }
  }
  positions
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
