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
//! A refused long option is refused in every abbreviation too: git's parse-options and `getopt_long`
//! (`file`, `ls`) accept any unambiguous prefix of a long option, so `git grep --op=<cmd>` is
//! `--open-files-in-pager`. Options that follow links are refused where a command walks folders: the
//! path check sees the folder it is given, not where a link inside it leads.
//!
//! Cargo is left out, although the design lists `cargo metadata` and `cargo tree`: both start `rustc`,
//! which a `.cargo/config.toml` or `rust-toolchain.toml` the agent writes in the worktree can replace
//! with any program, so they ask like any other command.

/// Programs that are known-safe with any arguments the path check accepts, unless their rules below
/// refuse some.
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
/// Git options the model may not pass: they set configuration, write files, or start a pager, a diff
/// program or `gpg` (`--show-signature`).
const GIT_REFUSED: [&str; 9] = [
  "-c",
  "--config-env",
  "--exec-path",
  "--output",
  "--open-files-in-pager",
  "--ext-diff",
  "--textconv",
  "--paginate",
  "--show-signature",
];
/// Git options that are a prefix of a refused option but name another one, which git matches exactly
/// before it looks for an abbreviation: `--text` is not `--textconv`.
const GIT_EXACT: [&str; 1] = ["--text"];
/// Short git options that are refused, also inside a cluster: `-O` (grep: start a pager; diff: read an
/// order file the path check cannot see when attached).
const GIT_REFUSED_SHORT: [char; 1] = ['O'];
/// The start of the `git log` format placeholders that verify a signature (`%G?`, `%GS`, …), which
/// starts `gpg`.
const GIT_SIGNATURE_PLACEHOLDER: &str = "%G";
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
/// Ripgrep options that start programs, decompress through them, or follow links.
const RG_REFUSED: [&str; 5] = ["--pre", "--pre-glob", "--search-zip", "--hostname-bin", "--follow"];
/// `rg -z` is `--search-zip`, `rg -L` is `--follow`.
const RG_REFUSED_SHORT: [char; 2] = ['z', 'L'];
/// `find` actions that run programs, delete, or write files, and `-follow`, which follows links.
const FIND_REFUSED: [&str; 10] = [
  "-exec", "-execdir", "-ok", "-okdir", "-delete", "-fprint", "-fprint0", "-fprintf", "-fls", "-follow",
];
/// The options BSD `find` (macOS) reads before its paths, also as a cluster (`-HL`); `-L` follows links.
const FIND_LEADING: &str = "EHLPXdsx";
/// `file --compile` (`-C`) writes a compiled magic file.
const FILE_REFUSED: [&str; 1] = ["--compile"];
const FILE_REFUSED_SHORT: [char; 1] = ['C'];
/// `ls --dereference` (`-L`) follows links, also those `-R` walks into.
const LS_REFUSED: [&str; 1] = ["--dereference"];
const LS_REFUSED_SHORT: [char; 1] = ['L'];

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
    "find" => !args.iter().any(|arg| find_refused(arg)),
    "file" => !args.iter().any(|arg| refused(arg, &FILE_REFUSED, &FILE_REFUSED_SHORT)),
    "ls" => !args.iter().any(|arg| refused(arg, &LS_REFUSED, &LS_REFUSED_SHORT)),
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

/// The program's name as the rules know it, or `None` when it is written with a folder. On Windows and
/// macOS, whose file systems find a program without regard to case, the name is matched without case;
/// on Windows also without `.exe`, as the program search finds it.
pub fn program_name(program: &str) -> Option<String> {
  if program.is_empty() || program.contains(['/', '\\']) {
    return None;
  }
  if cfg!(windows) {
    let lower = program.to_ascii_lowercase();
    return Some(lower.strip_suffix(".exe").map_or_else(|| lower.clone(), str::to_owned));
  }
  if cfg!(target_os = "macos") {
    return Some(program.to_ascii_lowercase());
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
  let refused_arg = |arg: &String| {
    arg.contains(GIT_SIGNATURE_PLACEHOLDER)
      || (!GIT_EXACT.contains(&arg.as_str()) && refused(arg, &GIT_REFUSED, &GIT_REFUSED_SHORT))
  };
  if rest.iter().any(refused_arg) {
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

/// Whether `find` `arg` is one of [`FIND_REFUSED`] or a cluster of leading options that holds `-L`.
fn find_refused(arg: &str) -> bool {
  FIND_REFUSED.contains(&arg)
    || arg
      .strip_prefix('-')
      .is_some_and(|flags| flags.contains('L') && flags.chars().all(|flag| FIND_LEADING.contains(flag)))
}

/// Whether `arg` is one of the `long` options or an abbreviation of one (also as `--option=value`), or
/// a short-option cluster holding one of `short`.
fn refused(arg: &str, long: &[&str], short: &[char]) -> bool {
  let name = arg.split_once('=').map_or(arg, |(name, _)| name);
  if long.contains(&name) {
    return true;
  }
  if let Some(abbreviation) = name.strip_prefix("--") {
    return !abbreviation.is_empty()
      && long
        .iter()
        .filter_map(|option| option.strip_prefix("--"))
        .any(|option| option.starts_with(abbreviation));
  }
  arg
    .strip_prefix('-')
    .is_some_and(|cluster| cluster.chars().any(|c| short.contains(&c)))
}
