//! What one simple command does by its program, subcommand and options (board `U97QE`), before the
//! paths it writes are placed: the tables of programs that only read, that build or test in the
//! worktree, that reach the network, that install software, that write or delete the files they name,
//! and that run a shell's script. A program none of them knows is Unknown: nothing is guessed.

use super::{git, packages};

/// What a command is by its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
  ReadOnly,
  /// Builds, tests or otherwise changes the worktree without naming the files.
  Worktree,
  Network,
  /// Rewrites or discards history.
  Destructive,
  /// Writes the files it names (or redirects to); `delete` when it removes them.
  Writes {
    delete: bool,
  },
  Unknown,
}

/// A command's kind with the words that name files it writes, and what else it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Judged {
  pub(super) kind: Kind,
  /// Words naming files the command writes or deletes.
  pub(super) writes: Vec<String>,
  /// It installs software or packages.
  pub(super) installs: bool,
  /// It reads a credential store (a keychain, Windows credentials, a password manager).
  pub(super) secret_store: bool,
}

impl Judged {
  pub(super) const fn of(kind: Kind) -> Self {
    Self {
      kind,
      writes: Vec::new(),
      installs: false,
      secret_store: false,
    }
  }

  const fn installing(mut self) -> Self {
    self.installs = true;
    self
  }
}

/// Programs that read files or print, and change nothing they are given.
const READS: [&str; 46] = [
  "cat",
  "head",
  "tail",
  "less",
  "more",
  "wc",
  "ls",
  "dir",
  "pwd",
  "echo",
  "printf",
  "which",
  "where",
  "whereis",
  "type",
  "file",
  "stat",
  "rg",
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
];

/// PowerShell's reading cmdlets and their aliases (matched without case).
const POWERSHELL_READS: [&str; 18] = [
  "get-content",
  "gc",
  "get-childitem",
  "gci",
  "select-string",
  "sls",
  "get-location",
  "gl",
  "test-path",
  "write-output",
  "write-host",
  "measure-object",
  "get-item",
  "gi",
  "resolve-path",
  "select-object",
  "set-location",
  "get-filehash",
];

/// Programs that write or delete each file they name (every argument that is not an option).
const WRITES_ARGS: [&str; 9] = [
  "mkdir", "touch", "chmod", "chown", "tee", "truncate", "new-item", "ni", "md",
];

/// Programs that remove each file they name.
const DELETES_ARGS: [&str; 8] = ["rm", "rmdir", "unlink", "remove-item", "ri", "del", "erase", "rd"];

/// Programs a prefix of which would also allow deleting outside the worktree or the temporary folder.
pub(super) const DESTRUCTIVE_PROGRAMS: [&str; 15] = [
  "rm",
  "rmdir",
  "unlink",
  "remove-item",
  "ri",
  "del",
  "erase",
  "rd",
  "mv",
  "move-item",
  "move",
  "mi",
  "find",
  "dd",
  "truncate",
];

/// Programs that write their last argument (or `-t <folder>`) and only read the others.
const COPIES: [&str; 6] = ["cp", "copy-item", "copy", "ln", "install", "cpi"];

/// Programs that move: the sources go away and the target is written, so every argument is written.
const MOVES: [&str; 5] = ["mv", "move-item", "move", "mi", "rename-item"];

/// PowerShell cmdlets that write the file of their `-Path` (or first argument).
const POWERSHELL_WRITES: [&str; 4] = ["set-content", "add-content", "out-file", "sc"];

/// Programs that reach the network by themselves.
const NETWORK: [&str; 15] = [
  "curl",
  "wget",
  "ssh",
  "scp",
  "sftp",
  "rsync",
  "ftp",
  "nc",
  "telnet",
  "ping",
  "gh",
  "invoke-webrequest",
  "iwr",
  "invoke-restmethod",
  "irm",
];

/// Build, test and run tools that work in the worktree, with the subcommands that do (any other is
/// Unknown or, for package managers, judged in [`packages`]).
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
    return Judged {
      secret_store: true,
      ..Judged::of(Kind::Unknown)
    };
  }
  if let Some(judged) = packages::judge(program, args) {
    return judged;
  }
  match program {
    "git" => git::judge(args),
    "sed" => sed(args),
    "find" => find(args),
    "sort" if has_option(args, &["-o", "--output"]) => Judged::of(Kind::Unknown),
    "curl" | "wget" => downloads(program, args),
    "env" if args.is_empty() => Judged::of(Kind::ReadOnly),
    program if READS.contains(&program) || POWERSHELL_READS.contains(&program) => Judged::of(Kind::ReadOnly),
    program if NETWORK.contains(&program) => Judged::of(Kind::Network),
    program if BUILDS.contains(&program) => Judged::of(Kind::Worktree),
    program if WRITES_ARGS.contains(&program) => writes(Kind::Writes { delete: false }, operands(args)),
    program if DELETES_ARGS.contains(&program) => writes(Kind::Writes { delete: true }, operands(args)),
    program if MOVES.contains(&program) => writes(Kind::Writes { delete: true }, operands(args)),
    program if COPIES.contains(&program) => writes(Kind::Writes { delete: false }, copy_target(args)),
    program if POWERSHELL_WRITES.contains(&program) => writes(
      Kind::Writes { delete: false },
      operands(args).into_iter().take(1).collect(),
    ),
    _ => Judged::of(Kind::Unknown),
  }
}

fn writes(kind: Kind, paths: Vec<String>) -> Judged {
  Judged {
    writes: paths,
    ..Judged::of(kind)
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

/// The target of a copy: `-t <folder>`, else the last operand.
fn copy_target(args: &[String]) -> Vec<String> {
  if let Some(at) = args.iter().position(|arg| arg == "-t" || arg == "--target-directory") {
    return args.get(at + 1).cloned().into_iter().collect();
  }
  operands(args).pop().into_iter().collect()
}

/// `sed` prints unless `-i` edits its files in place. A script that writes a file (`w`) or runs a
/// command (`e`) is not read.
fn sed(args: &[String]) -> Judged {
  let operands = operands(args);
  let script_given = has_option(args, &["-e", "--expression", "-f", "--file"]);
  let (script, files) = match (script_given, operands.split_first()) {
    (false, Some((script, files))) => (Some(script.as_str()), files.to_vec()),
    (..) => (None, operands.clone()),
  };
  if script_given || script.is_some_and(|script| script.contains(['w', 'W', 'e'])) {
    return Judged::of(Kind::Unknown);
  }
  if has_option(args, &["-i", "--in-place"]) {
    return writes(Kind::Writes { delete: false }, files);
  }
  Judged::of(Kind::ReadOnly)
}

/// `find` lists unless it deletes or runs a program for what it finds.
fn find(args: &[String]) -> Judged {
  if args.iter().any(|arg| {
    matches!(
      arg.as_str(),
      "-exec" | "-execdir" | "-ok" | "-okdir" | "-fprint" | "-fprintf" | "-fls"
    )
  }) {
    return Judged::of(Kind::Unknown);
  }
  if args.iter().any(|arg| arg == "-delete") {
    let roots = args
      .iter()
      .take_while(|arg| !arg.starts_with('-'))
      .cloned()
      .collect::<Vec<_>>();
    let roots = if roots.is_empty() { vec![".".to_owned()] } else { roots };
    return writes(Kind::Writes { delete: true }, roots);
  }
  Judged::of(Kind::ReadOnly)
}

/// `curl` and `wget` reach the network, and write what they fetch where they are told to.
fn downloads(program: &str, args: &[String]) -> Judged {
  let options: &[&str] = if program == "curl" {
    &["-o", "--output"]
  } else {
    &["-O", "--output-document", "-P", "--directory-prefix"]
  };
  let target = args
    .iter()
    .position(|arg| options.contains(&arg.as_str()))
    .and_then(|at| args.get(at + 1))
    .cloned();
  let remote_name = program == "curl" && has_option(args, &["-O", "--remote-name"]);
  let mut paths: Vec<String> = target.into_iter().collect();
  if remote_name || (program == "wget" && paths.is_empty()) {
    paths.push(".".to_owned());
  }
  Judged {
    writes: paths,
    ..Judged::of(Kind::Network)
  }
}

/// A package manager's or toolchain's verdict that also installs.
pub(super) const fn installs(kind: Kind) -> Judged {
  Judged::of(kind).installing()
}
