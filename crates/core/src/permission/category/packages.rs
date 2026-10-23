//! Package managers and toolchains by their subcommand: installing or adding software reaches the
//! network and installs (`cargo add`, `npm install`, `pip install`, `npx`, a global install), building,
//! testing and running the project work in the worktree (`cargo test`, `npm run`), what only prints is
//! Read-only, and what looks packages up in a registry is Network. Any other subcommand is Unknown. A
//! build, test or read must name only paths inside (`--target-dir`, `--manifest-path`, `-o`), else it
//! is Unknown.
//!
//! A lookup or a read that keeps what it read in the tool's own cache outside the worktree (Cargo's registry index
//! and downloads, npm's and pip's caches), or may bring a lock file up to date, writes all the same
//! ([`Judged::updates_cache`], the person's decision of 2026-10-23): only `cargo search` asks the registry and keeps
//! nothing, and Cargo resolves without writing only `--frozen` (or `--offline --locked`), `cargo metadata` not at all
//! with `--no-deps`; pip keeps nothing with `--no-cache-dir`.

use super::{
  build_output::BuildWords,
  install::InstallSource,
  programs::{Judged, Kind, has_option},
};

/// A program's subcommands by what they do.
struct Manager {
  programs: &'static [&'static str],
  /// Where its installs come from, as a person knows it; `None` for system and version managers,
  /// which are named by their program.
  registry: Option<&'static str>,
  installs: &'static [&'static str],
  builds: &'static [&'static str],
  reads: &'static [&'static str],
  /// Of `reads`, those that resolve the project's dependencies, which may refresh the registry's cache and the lock
  /// file unless told not to ([`resolves_quietly`]).
  resolves: &'static [&'static str],
  /// Subcommands that only read, from the package registry.
  looks_up: &'static [&'static str],
  /// Of `looks_up`, those that keep nothing of what they read.
  keeps_nothing: &'static [&'static str],
}

const MANAGERS: [Manager; 11] = [
  Manager {
    programs: &["cargo"],
    registry: Some("crates.io"),
    installs: &["add", "install", "update", "fetch", "generate-lockfile"],
    builds: &[
      "build", "check", "test", "clippy", "fmt", "run", "doc", "bench", "clean", "b", "c", "t", "r", "nextest",
    ],
    reads: &[
      "metadata",
      "tree",
      "--version",
      "-V",
      "version",
      "locate-project",
      "pkgid",
      "verify-project",
    ],
    resolves: &["metadata", "tree", "pkgid"],
    looks_up: &["search", "info"],
    keeps_nothing: &["search"],
  },
  Manager {
    programs: &["npm", "pnpm", "yarn", "bun"],
    registry: Some("npm"),
    installs: &[
      "install", "i", "add", "ci", "update", "upgrade", "up", "dlx", "x", "exec", "create", "init", "link",
    ],
    builds: &["run", "test", "start", "build", "lint", "t", "pack"],
    reads: &["ls", "list", "why", "--version", "-v"],
    resolves: &[],
    looks_up: &["view", "outdated", "audit"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["pip", "pip3", "pipx", "uv", "poetry", "conda", "mamba"],
    registry: Some("PyPI"),
    installs: &[
      "install", "add", "sync", "lock", "update", "upgrade", "run", "tool", "pip", "inject", "create",
    ],
    builds: &["build"],
    reads: &["list", "show", "freeze", "--version", "tree", "check", "info"],
    resolves: &[],
    looks_up: &["search", "index"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["go"],
    registry: Some("Go modules"),
    installs: &["get", "install", "mod"],
    builds: &["build", "test", "vet", "fmt", "run", "generate"],
    reads: &["version", "env", "doc"],
    resolves: &[],
    looks_up: &["list"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["dotnet"],
    registry: Some("NuGet"),
    installs: &["add", "tool", "restore", "new", "workload"],
    builds: &["build", "test", "run", "format", "publish", "pack", "clean"],
    reads: &["--version", "--info", "--list-sdks"],
    resolves: &[],
    looks_up: &["list"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["gem", "bundle", "bundler", "composer"],
    registry: Some("RubyGems"),
    installs: &["install", "update", "add", "require", "exec"],
    builds: &["test"],
    reads: &["list", "show", "--version"],
    resolves: &[],
    looks_up: &["outdated"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["brew", "apt", "apt-get", "yum", "dnf", "pacman", "apk", "port"],
    registry: None,
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
    resolves: &[],
    looks_up: &["search"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["winget", "choco", "scoop"],
    registry: None,
    installs: &["install", "upgrade", "update", "add"],
    builds: &[],
    reads: &["list", "--version"],
    resolves: &[],
    looks_up: &["search", "show"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["rustup"],
    registry: Some("rustup"),
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
    resolves: &[],
    looks_up: &[],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["deno"],
    registry: Some("deno"),
    installs: &["install", "add", "cache", "upgrade", "run"],
    builds: &["test", "check", "fmt", "lint"],
    reads: &["--version"],
    resolves: &[],
    looks_up: &["info"],
    keeps_nothing: &[],
  },
  Manager {
    programs: &["nvm", "volta", "asdf", "mise", "fnm"],
    registry: None,
    installs: &["install", "use", "pin", "plugin"],
    builds: &[],
    reads: &["list", "ls", "current", "--version"],
    resolves: &[],
    looks_up: &[],
    keeps_nothing: &[],
  },
];

/// Programs that fetch a package and run it: they always install.
const FETCH_AND_RUN: [&str; 4] = ["npx", "pnpx", "bunx", "uvx"];

/// The verdict for package manager or toolchain `program` with `args`; `None` for any other program.
pub(super) fn judge(program: &str, args: &[String]) -> Option<Judged> {
  if FETCH_AND_RUN.contains(&program) {
    let registry = if program == "uvx" { "PyPI" } else { "npm" };
    return Some(
      Judged::of(Kind::Network)
        .installing()
        .installed_from(InstallSource::of(registry, args)),
    );
  }
  let manager = MANAGERS.iter().find(|manager| manager.programs.contains(&program))?;
  let Some((at, sub)) = subcommand(args) else {
    // `yarn` alone installs; any other manager without a subcommand prints its help.
    return Some(if program == "yarn" {
      Judged::of(Kind::Network)
        .installing()
        .installed_from(InstallSource::of(manager.registry.unwrap_or(program), &[]))
    } else {
      Judged::of(Kind::ReadOnly)
    });
  };
  let rest = &args[at + 1..];
  // A Cargo setting given on the command line can name a program it starts (`build.rustc-wrapper`).
  if program == "cargo"
    && args
      .iter()
      .any(|arg| arg == "--config" || arg.starts_with("--config=") || arg.starts_with("-Z"))
  {
    return Some(Judged::unknown());
  }
  // Installs and builds run the packages' and the worktree's own code whatever their arguments, so a
  // prefix of them allows nothing more; `clean` deletes the folder it is given.
  let judged = if has_option(args, &["-g", "--global", "--location=global"]) || manager.installs.contains(&sub) {
    Judged::of(Kind::Network)
      .installing()
      .prefix_stays()
      .installed_from(InstallSource::of(manager.registry.unwrap_or(program), rest))
  } else if manager.builds.contains(&sub) {
    let mut judged = Judged::of(Kind::Worktree).inside_all(rest);
    if program == "cargo" {
      // `--emit=metadata=<path>`: the path a value names after its own `=` must lie inside too.
      judged.inside.extend(
        rest
          .iter()
          .filter_map(|arg| arg.rsplit_once('=').map(|(_, path)| path.to_owned())),
      );
      judged.build_words = BuildWords::of(&args[..at], sub, rest);
    }
    if sub == "clean" { judged } else { judged.prefix_stays() }
  } else if manager.reads.contains(&sub) {
    let judged = Judged::of(Kind::ReadOnly).inside_all(rest);
    if manager.resolves.contains(&sub) && !resolves_quietly(sub, args) {
      judged.updating_cache()
    } else {
      judged
    }
  } else if manager.looks_up.contains(&sub) {
    let judged = Judged::of(Kind::Network).inside_all(rest).looking_up();
    let keeps_nothing = manager.keeps_nothing.contains(&sub)
      || (matches!(program, "pip" | "pip3") && args.iter().any(|arg| arg == "--no-cache-dir"));
    if keeps_nothing { judged } else { judged.updating_cache() }
  } else {
    // `python -m pip install` reaches pip through python, which is not read.
    Judged::unknown()
  };
  Some(judged.subcommand(at, sub.to_owned()))
}

/// Whether Cargo's `sub` with `args` resolves the dependencies without writing: from what it has already, keeping
/// the lock file as it is (`--frozen`, or `--offline` with `--locked`), or, for `cargo metadata --no-deps`, not at
/// all. Only Cargo's own options count, not what follows `--`.
fn resolves_quietly(sub: &str, args: &[String]) -> bool {
  let given = |option: &str| args.iter().take_while(|arg| *arg != "--").any(|arg| arg == option);
  given("--frozen") || (given("--offline") && given("--locked")) || (sub == "metadata" && given("--no-deps"))
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
