//! Package managers and toolchains by their subcommand: installing or adding software reaches the
//! network and installs (`cargo add`, `npm install`, `pip install`, `npx`, a global install), building,
//! testing and running the project work in the worktree (`cargo test`, `npm run`), and what only
//! prints is Read-only. Any other subcommand is Unknown.

use super::programs::{Judged, Kind, has_option, installs};

/// A program's subcommands by what they do.
struct Manager {
  programs: &'static [&'static str],
  installs: &'static [&'static str],
  builds: &'static [&'static str],
  reads: &'static [&'static str],
}

const MANAGERS: [Manager; 11] = [
  Manager {
    programs: &["cargo"],
    installs: &["add", "install", "update", "fetch", "generate-lockfile"],
    builds: &[
      "build", "check", "test", "clippy", "fmt", "run", "doc", "bench", "clean", "b", "c", "t", "r", "nextest",
    ],
    reads: &[
      "metadata",
      "tree",
      "--version",
      "-V",
      "search",
      "locate-project",
      "pkgid",
    ],
  },
  Manager {
    programs: &["npm", "pnpm", "yarn", "bun"],
    installs: &[
      "install", "i", "add", "ci", "update", "upgrade", "up", "dlx", "x", "exec", "create", "init", "link",
    ],
    builds: &["run", "test", "start", "build", "lint", "t"],
    reads: &[
      "ls",
      "list",
      "view",
      "outdated",
      "why",
      "--version",
      "-v",
      "audit",
      "pack",
    ],
  },
  Manager {
    programs: &["pip", "pip3", "pipx", "uv", "poetry", "conda", "mamba"],
    installs: &[
      "install", "add", "sync", "lock", "update", "upgrade", "run", "tool", "pip", "inject", "create",
    ],
    builds: &["build"],
    reads: &["list", "show", "freeze", "--version", "tree", "check", "info"],
  },
  Manager {
    programs: &["go"],
    installs: &["get", "install", "mod"],
    builds: &["build", "test", "vet", "fmt", "run", "generate"],
    reads: &["version", "env", "list", "doc"],
  },
  Manager {
    programs: &["dotnet"],
    installs: &["add", "tool", "restore", "new", "workload"],
    builds: &["build", "test", "run", "format", "publish", "pack", "clean"],
    reads: &["--version", "--info", "--list-sdks", "list"],
  },
  Manager {
    programs: &["gem", "bundle", "bundler", "composer"],
    installs: &["install", "update", "add", "require", "exec"],
    builds: &["test"],
    reads: &["list", "show", "outdated", "--version"],
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
    reads: &["list", "info", "search", "--version", "show", "policy"],
  },
  Manager {
    programs: &["winget", "choco", "scoop"],
    installs: &["install", "upgrade", "update", "add"],
    builds: &[],
    reads: &["list", "show", "search", "--version"],
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
  },
  Manager {
    programs: &["deno"],
    installs: &["install", "add", "cache", "upgrade", "run"],
    builds: &["test", "check", "fmt", "lint"],
    reads: &["--version", "info"],
  },
  Manager {
    programs: &["nvm", "volta", "asdf", "mise", "fnm"],
    installs: &["install", "use", "pin", "plugin"],
    builds: &[],
    reads: &["list", "ls", "current", "--version"],
  },
];

/// Programs that fetch a package and run it: they always install.
const FETCH_AND_RUN: [&str; 4] = ["npx", "pnpx", "bunx", "uvx"];

/// The verdict for package manager or toolchain `program` with `args`; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  if FETCH_AND_RUN.contains(&program) {
    return Some(installs(Kind::Network));
  }
  let manager = MANAGERS.iter().find(|manager| manager.programs.contains(&program))?;
  let Some(sub) = subcommand(args) else {
    // `yarn` alone installs; any other manager without a subcommand prints its help.
    return Some(if program == "yarn" {
      installs(Kind::Network)
    } else {
      Judged::of(Kind::ReadOnly)
    });
  };
  if has_option(args, &["-g", "--global", "--location=global"]) || manager.installs.contains(&sub) {
    return Some(installs(Kind::Network));
  }
  if manager.builds.contains(&sub) {
    return Some(Judged::of(Kind::Worktree));
  }
  if manager.reads.contains(&sub) {
    return Some(Judged::of(Kind::ReadOnly));
  }
  // `python -m pip install` reaches pip through python, which is judged as a script runner.
  Some(Judged::of(Kind::Unknown))
}

/// The first argument that is not an option, or a version option that stands for the subcommand.
fn subcommand(args: &[String]) -> Option<&str> {
  args.iter().map(String::as_str).find(|arg| {
    !arg.starts_with('-')
      || matches!(
        *arg,
        "--version" | "-V" | "-v" | "--info" | "--list-sdks" | "-S" | "-s" | "-Syu"
      )
  })
}

/// Whether `program` (as [`super::program_key`] gives it) has subcommands, so a rule names one.
pub(super) fn has_subcommands(program: &str) -> bool {
  matches!(program, "git" | "gh") || MANAGERS.iter().any(|manager| manager.programs.contains(&program))
}
