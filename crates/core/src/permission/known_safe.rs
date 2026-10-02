//! Known-safe read-only commands (`docs/architecture/native-agent.md` §5.2): rules over a command's
//! program and arguments, not prefixes, for commands that read files but change none and start no other
//! program, modelled on Codex's `is_known_safe_command`. The native agent runs them without asking once
//! its allowed list has an entry.
//!
//! The rules judge the words alone. The caller must also check that every argument naming an existing
//! path lies in the readable scope ([`path_candidates`]), which stops `cat ~/.ssh/id_rsa` written as an
//! absolute path. A program written with a folder (`./cat`) is never known-safe: it could be a file the
//! agent wrote.
//!
//! Cargo is left out, although the design lists `cargo metadata` and `cargo tree`: both start `rustc`,
//! which a `.cargo/config.toml` or `rust-toolchain.toml` the agent writes in the worktree can replace
//! with any program, so they ask like any other command.

/// Programs that are known-safe with any arguments the path check accepts.
const PLAIN: [&str; 11] = [
  "ls", "cat", "head", "tail", "wc", "pwd", "echo", "which", "where", "file", "stat",
];
/// Programs whose `--version` alone is known-safe, besides [`PLAIN`].
const VERSIONED: [&str; 3] = ["git", "rg", "find"];
/// Git subcommands that only read.
const GIT_READ: [&str; 9] = [
  "status",
  "log",
  "diff",
  "show",
  "rev-parse",
  "ls-files",
  "branch",
  "blame",
  "grep",
];
/// Git options the model may not pass: they set configuration, write files, or start a pager or diff
/// program.
const GIT_REFUSED: [&str; 8] = [
  "-c",
  "--config-env",
  "--exec-path",
  "--output",
  "--open-files-in-pager",
  "--ext-diff",
  "--textconv",
  "--paginate",
];
/// Short git options that are refused, also inside a cluster: `-O` (grep: start a pager; diff: read an
/// order file the path check cannot see when attached).
const GIT_REFUSED_SHORT: [char; 1] = ['O'];
/// `git branch` options that only list.
const GIT_BRANCH_LIST: [&str; 10] = [
  "--list",
  "-l",
  "--show-current",
  "-a",
  "--all",
  "-r",
  "--remotes",
  "-v",
  "-vv",
  "--verbose",
];
/// Ripgrep options that start programs or decompress through them.
const RG_REFUSED: [&str; 4] = ["--pre", "--pre-glob", "--search-zip", "--hostname-bin"];
/// `rg -z` is `--search-zip`.
const RG_REFUSED_SHORT: [char; 1] = ['z'];
/// `find` actions that run programs, delete, or write files.
const FIND_REFUSED: [&str; 9] = [
  "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprint0", "-fprintf", "-fls",
];
/// `file` options that write a compiled magic file.
const FILE_REFUSED: [&str; 2] = ["-C", "--compile"];

/// Whether `argv` (program first) is a known-safe read-only command by its words.
pub fn is_known_safe(argv: &[String]) -> bool {
  let Some((program, args)) = argv.split_first() else {
    return false;
  };
  let Some(program) = program_name(program) else {
    return false;
  };
  if matches!(args, [only] if only == "--version")
    && (PLAIN.contains(&program.as_str()) || VERSIONED.contains(&program.as_str()))
  {
    return true;
  }
  match program.as_str() {
    "git" => git(args),
    "rg" => !args.iter().any(|arg| refused(arg, &RG_REFUSED, &RG_REFUSED_SHORT)),
    "find" => !args.iter().any(|arg| FIND_REFUSED.contains(&arg.as_str())),
    "file" => !args.iter().any(|arg| refused(arg, &FILE_REFUSED, &[])),
    other => PLAIN.contains(&other),
  }
}

/// The parts of a known-safe command's arguments that may name a file: every argument, the value after
/// `=` of an option (`--ignore-file=…`) and the value attached to a short option (`-f…`).
pub fn path_candidates(argv: &[String]) -> Vec<&str> {
  let mut candidates = Vec::new();
  for arg in argv.iter().skip(1) {
    candidates.push(arg.as_str());
    if let Some(option) = arg.strip_prefix("--") {
      if let Some((_, value)) = option.split_once('=') {
        candidates.push(value);
      }
    } else if let Some(attached) = arg.strip_prefix('-').and_then(|rest| rest.get(1..))
      && !attached.is_empty()
    {
      candidates.push(attached);
    }
  }
  candidates
}

/// The program's name as the rules know it, or `None` when it is written with a folder. On Windows the
/// name is matched without case and without `.exe`, as the program search finds it.
pub fn program_name(program: &str) -> Option<String> {
  if program.is_empty() || program.contains(['/', '\\']) {
    return None;
  }
  if cfg!(windows) {
    let lower = program.to_ascii_lowercase();
    return Some(lower.strip_suffix(".exe").map_or_else(|| lower.clone(), str::to_owned));
  }
  Some(program.to_owned())
}

fn git(args: &[String]) -> bool {
  let Some((subcommand, rest)) = args.split_first() else {
    return false;
  };
  if !GIT_READ.contains(&subcommand.as_str()) {
    return false;
  }
  if rest.iter().any(|arg| refused(arg, &GIT_REFUSED, &GIT_REFUSED_SHORT)) {
    return false;
  }
  subcommand != "branch" || branch_lists(rest)
}

/// Whether `git branch` with `args` only lists branches: every option lists, and names are patterns
/// only with `--list`.
fn branch_lists(args: &[String]) -> bool {
  let listing = args.iter().any(|arg| arg == "--list" || arg == "-l");
  args.iter().all(|arg| {
    if arg.starts_with('-') {
      GIT_BRANCH_LIST.contains(&arg.as_str())
    } else {
      listing
    }
  })
}

/// Whether `arg` is one of the `long` options (also as `--option=value`) or a short-option cluster
/// holding one of `short`.
fn refused(arg: &str, long: &[&str], short: &[char]) -> bool {
  let name = arg.split_once('=').map_or(arg, |(name, _)| name);
  if long.contains(&name) {
    return true;
  }
  match arg.strip_prefix('-') {
    Some(cluster) if !cluster.starts_with('-') => cluster.chars().any(|c| short.contains(&c)),
    _ => false,
  }
}
