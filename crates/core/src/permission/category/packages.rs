//! Package managers and toolchains by their subcommand: installing or adding software reaches the
//! network and installs (`cargo add`, `npm install`, `pip install`, `npx`, a global install), building,
//! testing and running the project work in the worktree (`cargo test`, `npm run`), what only prints is
//! Read-only, and what looks packages up in a registry is Network. Any other subcommand is Unknown. A
//! build, test or read must name only paths inside (`--target-dir`, `--manifest-path`, `-o`), else it
//! is Unknown.

use super::programs::{Judged, Kind, has_option};

/// A program's subcommands by what they do.
struct Manager {
  programs: &'static [&'static str],
  installs: &'static [&'static str],
  builds: &'static [&'static str],
  reads: &'static [&'static str],
  /// Subcommands that only read, from the package registry.
  looks_up: &'static [&'static str],
}

const MANAGERS: [Manager; 11] = [
  Manager {
    programs: &["cargo"],
    installs: &["add", "install", "update", "fetch", "generate-lockfile"],
    builds: &[
      "build", "check", "test", "clippy", "fmt", "run", "doc", "bench", "clean", "b", "c", "t", "r", "nextest",
    ],
    reads: &["metadata", "tree", "--version", "-V", "locate-project", "pkgid"],
    looks_up: &["search"],
  },
  Manager {
    programs: &["npm", "pnpm", "yarn", "bun"],
    installs: &[
      "install", "i", "add", "ci", "update", "upgrade", "up", "dlx", "x", "exec", "create", "init", "link",
    ],
    builds: &["run", "test", "start", "build", "lint", "t", "pack"],
    reads: &["ls", "list", "why", "--version", "-v"],
    looks_up: &["view", "outdated", "audit"],
  },
  Manager {
    programs: &["pip", "pip3", "pipx", "uv", "poetry", "conda", "mamba"],
    installs: &[
      "install", "add", "sync", "lock", "update", "upgrade", "run", "tool", "pip", "inject", "create",
    ],
    builds: &["build"],
    reads: &["list", "show", "freeze", "--version", "tree", "check", "info"],
    looks_up: &["search"],
  },
  Manager {
    programs: &["go"],
    installs: &["get", "install", "mod"],
    builds: &["build", "test", "vet", "fmt", "run", "generate"],
    reads: &["version", "env", "doc"],
    looks_up: &["list"],
  },
  Manager {
    programs: &["dotnet"],
    installs: &["add", "tool", "restore", "new", "workload"],
    builds: &["build", "test", "run", "format", "publish", "pack", "clean"],
    reads: &["--version", "--info", "--list-sdks"],
    looks_up: &["list"],
  },
  Manager {
    programs: &["gem", "bundle", "bundler", "composer"],
    installs: &["install", "update", "add", "require", "exec"],
    builds: &["test"],
    reads: &["list", "show", "--version"],
    looks_up: &["outdated"],
  },
  Manager {
    programs: &["brew", "apt", "apt-get", "yum", "dnf", "pacman", "apk", "port"],
    installs: &[
      "install",
      "reinstall",
      "upgrade",
      "update",
      "tap",
      "add",
      "-s",
      "-S",
      "-Syu",
    ],
    builds: &[],
    reads: &["list", "info", "--version", "show", "policy"],
    looks_up: &["search"],
  },
  Manager {
    programs: &["winget", "choco", "scoop"],
    installs: &["install", "upgrade", "update", "add"],
    builds: &[],
    reads: &["list", "--version"],
    looks_up: &["search", "show"],
  },
  Manager {
    programs: &["rustup"],
    installs: &[
      "install",
      "update",
      "toolchain",
      "component",
      "target",
      "default",
      "override",
    ],
    builds: &[],
    reads: &["show", "--version", "which"],
    looks_up: &[],
  },
  Manager {
    programs: &["deno"],
    installs: &["install", "add", "cache", "upgrade", "run"],
    builds: &["test", "check", "fmt", "lint"],
    reads: &["--version"],
    looks_up: &["info"],
  },
  Manager {
    programs: &["nvm", "volta", "asdf", "mise", "fnm"],
    installs: &["install", "use", "pin", "plugin"],
    builds: &[],
    reads: &["list", "ls", "current", "--version"],
    looks_up: &[],
  },
];

/// Programs that fetch a package and run it: they always install.
const FETCH_AND_RUN: [&str; 4] = ["npx", "pnpx", "bunx", "uvx"];

/// The verdict for package manager or toolchain `program` with `args`; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  if FETCH_AND_RUN.contains(&program) {
    return Some(Judged::of(Kind::Network).installing());
  }
  let manager = MANAGERS.iter().find(|manager| manager.programs.contains(&program))?;
  let Some((at, sub)) = subcommand(args) else {
    // `yarn` alone installs; any other manager without a subcommand prints its help.
    return Some(if program == "yarn" {
      Judged::of(Kind::Network).installing()
    } else {
      Judged::of(Kind::ReadOnly)
    });
  };
  let rest = &args[at + 1..];
  // Installs and builds run the packages' and the worktree's own code whatever their arguments, so a
  // prefix of them allows nothing more; `clean` deletes the folder it is given.
  let judged = if has_option(args, &["-g", "--global", "--location=global"]) || manager.installs.contains(&sub) {
    Judged::of(Kind::Network).installing().prefix_stays()
  } else if manager.builds.contains(&sub) {
    let judged = Judged::of(Kind::Worktree).inside_all(rest);
    if sub == "clean" { judged } else { judged.prefix_stays() }
  } else if manager.reads.contains(&sub) {
    Judged::of(Kind::ReadOnly).inside_all(rest)
  } else if manager.looks_up.contains(&sub) {
    Judged::of(Kind::Network).inside_all(rest)
  } else {
    // `python -m pip install` reaches pip through python, which is not read.
    Judged::unknown()
  };
  Some(judged.subcommand(at, sub.to_owned()))
}

/// The first argument that is not an option or a toolchain (`cargo +nightly fmt`), or a version option
/// that stands for the subcommand, and where it is.
fn subcommand(args: &[String]) -> Option<(usize, &str)> {
  args.iter().map(String::as_str).enumerate().find(|(_, arg)| {
    !arg.starts_with(['-', '+'])
      || matches!(
        *arg,
        "--version" | "-V" | "-v" | "--info" | "--list-sdks" | "-S" | "-s" | "-Syu"
      )
  })
}
