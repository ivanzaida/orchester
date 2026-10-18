//! The words of a Cargo build that tell whether it writes only build output, and where. Only Cargo's own names of
//! the subcommands count: `b`, `c` and `t` are aliases, which a configuration's `[alias]` may name otherwise. What
//! follows `--` goes to the test binary or the linter, which may write anything they are told to (`--logfile`,
//! `--emit`, `-o`): only the options listed here, which write nothing, keep a build one.

/// Cargo's subcommands that write only build output, by their full names.
const SUBCOMMANDS: [&str; 6] = ["build", "check", "test", "clippy", "doc", "bench"];

/// Their options that change sources, or start another program.
const REFUSED: [&str; 6] = [
  "--fix",
  "--allow-dirty",
  "--allow-staged",
  "--allow-no-vcs",
  "--broken-code",
  "--open",
];

/// The test harness's options after `--` that write nothing: alone, and with a value.
const TEST_FLAGS: [&str; 8] = [
  "--nocapture",
  "--exact",
  "--ignored",
  "--include-ignored",
  "-q",
  "--quiet",
  "--show-output",
  "--list",
];
const TEST_VALUED: [&str; 4] = ["--test-threads", "--skip", "--color", "--format"];

/// The linter's options after `--` that only set a lint's level, each with its lint; the short ones first.
const LINT_LEVELS: [&str; 8] = ["-D", "-W", "-A", "-F", "--deny", "--warn", "--allow", "--forbid"];

/// The words of a Cargo build that tell where its output goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct BuildWords {
  /// The value of `--target-dir`, as written.
  pub(super) target_dir: Option<String>,
  /// `--manifest-path` names the package, whose workspace may lie elsewhere than the folder Cargo runs in.
  pub(super) manifest: bool,
  /// `clippy`, which is no built-in subcommand: an `[alias]` of a configuration may stand in its place.
  pub(super) clippy: bool,
}

impl BuildWords {
  /// The words of `cargo <sub> <rest>` when it writes only build output; `None` for another subcommand or an alias,
  /// a refused option anywhere, `--target-dir` given twice or without a value, or an argument after `--` that is not
  /// listed for the subcommand.
  pub(in super::super) fn of(sub: &str, rest: &[String]) -> Option<Self> {
    if !SUBCOMMANDS.contains(&sub) {
      return None;
    }
    let name = |arg: &str| arg.split_once('=').map_or(arg, |(name, _)| name).to_owned();
    if rest.iter().any(|arg| REFUSED.contains(&name(arg).as_str())) {
      return None;
    }
    let (cargo, trailing) = match rest.iter().position(|arg| arg == "--") {
      Some(at) => (&rest[..at], &rest[at + 1..]),
      None => (rest, &[][..]),
    };
    if !trailing_writes_nothing(sub, trailing) {
      return None;
    }
    let mut words = Self {
      target_dir: None,
      manifest: false,
      clippy: sub == "clippy",
    };
    let mut args = cargo.iter();
    while let Some(arg) = args.next() {
      match arg.split_once('=') {
        Some(("--target-dir", value)) => words.target(value.to_owned())?,
        None if arg == "--target-dir" => words.target(args.next()?.clone())?,
        Some(("--manifest-path", _)) => words.manifest = true,
        None if arg == "--manifest-path" => words.manifest = true,
        _ => {}
      }
    }
    Some(words)
  }

  /// Notes `--target-dir`'s `value`; `None` when it was given before.
  fn target(&mut self, value: String) -> Option<()> {
    self.target_dir.replace(value).is_none().then_some(())
  }
}

/// Whether the arguments after `--` of `cargo <sub>` are only ones that write nothing: for a test or bench, name
/// filters and the harness's listed options; for clippy, lint levels; for any other subcommand, none.
fn trailing_writes_nothing(sub: &str, trailing: &[String]) -> bool {
  let (flags, valued): (&[&str], &[&str]) = match sub {
    "test" | "bench" => (&TEST_FLAGS, &TEST_VALUED),
    "clippy" => (&[], &LINT_LEVELS),
    _ => return trailing.is_empty(),
  };
  let mut args = trailing.iter();
  while let Some(arg) = args.next() {
    let listed = if !arg.starts_with('-') {
      // A test's name filter; for the linter a word is only ever a level's lint.
      sub != "clippy"
    } else if flags.contains(&arg.as_str()) {
      true
    } else if let Some((option, value)) = arg.split_once('=') {
      valued.contains(&option) && plain(value)
    } else if valued.contains(&arg.as_str()) {
      args.next().is_some_and(|value| plain(value))
    } else {
      sub == "clippy" && attached_level(arg)
    };
    if !listed {
      return false;
    }
  }
  true
}

/// Whether `arg` is a short lint level with its lint attached (`-Dwarnings`).
fn attached_level(arg: &str) -> bool {
  LINT_LEVELS[..4]
    .iter()
    .any(|level| arg.strip_prefix(level).is_some_and(plain))
}

/// Whether `value` is a plain value, not an option of its own.
fn plain(value: &str) -> bool {
  !value.is_empty() && !value.starts_with('-')
}
