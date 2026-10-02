//! Known-safe read-only commands (`docs/architecture/native-agent.md` §5.2): rules over a command's
//! program and arguments, not prefixes, for commands that read files and start no other program,
//! modelled on Codex's `is_known_safe_command`. The native agent runs them without asking once its
//! allowed list has an entry.
//!
//! Every program, and every git subcommand, admits only the options it lists ([`options::Options`]);
//! any other option, an abbreviation of a listed one included, makes the command ask. Left out are the
//! options that start a program, write a file, read a file of paths, or follow links into folders the
//! path check does not see.
//!
//! The rules judge the words alone. The caller must also check that every argument that may name a
//! file lies in the readable scope ([`readable_arguments`]), which stops `cat ~/.ssh/id_rsa` written as
//! an absolute path. A program written with a folder (`./cat`) is never known-safe: it could be a file
//! the agent wrote.
//!
//! Git keeps its own files: `git diff` refreshes the file stamps cached in `.git/index`, and `git
//! status` would too without the `--no-optional-locks` it is started with. Neither changes what the
//! index or the worktree holds.
//!
//! Cargo is left out, although the design lists `cargo metadata` and `cargo tree`: both start `rustc`,
//! which a `.cargo/config.toml` or `rust-toolchain.toml` the agent writes in the worktree can replace
//! with any program, so they ask like any other command.

mod git;
mod head_tail;
mod options;
mod tables;

pub use self::git::SUBCOMMANDS as GIT_SUBCOMMANDS;

/// Every program the table knows, each known-safe with `--version` alone too.
pub const PROGRAMS: [&str; 14] = [
  "ls", "cat", "head", "tail", "wc", "pwd", "echo", "which", "file", "stat", "rg", "git", "find", "where",
];

/// The arguments of a known-safe command (program first) that may name a file: every argument, the
/// value after `=` of a long option and the value attached to a short option (`-f…`), also inside a
/// cluster (`-nf…`). `None` when the command is not known-safe by its words.
pub fn readable_arguments(argv: &[String]) -> Option<Vec<&str>> {
  let (program, args) = argv.split_first()?;
  let program = program_name(program)?;
  if !PROGRAMS.contains(&program.as_str()) {
    return None;
  }
  if matches!(args, [only] if only == "--version") {
    return Some(Vec::new());
  }
  match program.as_str() {
    "git" => git::readable(args),
    "head" | "tail" => head_tail::readable(args),
    "find" => tables::find(args),
    "where" => tables::where_(args),
    name => tables::OPTION_PROGRAMS
      .iter()
      .find_map(|(known, options)| (*known == name).then_some(options))?
      .readable(args),
  }
}

/// The program's name as the rules know it, or `None` when it is written with a folder. On Windows and
/// macOS, whose file systems find a program without regard to case by default, the name is matched
/// without case; on Windows also without `.exe`, as the program search finds it. On a case-sensitive
/// macOS volume a program literally named `Git` earlier on `PATH` would then be taken for `git`.
pub fn program_name(program: &str) -> Option<String> {
  if program.is_empty() || program.contains(['/', '\\']) {
    return None;
  }
  if cfg!(windows) {
    let mut lower = program.to_ascii_lowercase();
    if lower.ends_with(".exe") {
      lower.truncate(lower.len() - ".exe".len());
    }
    return Some(lower);
  }
  if cfg!(target_os = "macos") {
    return Some(program.to_ascii_lowercase());
  }
  Some(program.to_owned())
}
