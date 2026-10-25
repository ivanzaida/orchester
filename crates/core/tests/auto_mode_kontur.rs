//! Auto mode out of the box (ORC-95): the read-only lines agents of a fresh project (Kontur) ran, which auto mode had
//! held for the person as unreadable in a step that only reads with the network off. Windows sets no `HOME`, so `~`
//! is read as Git Bash reads it there, the profile; a line made of known reads is read to the end and goes to the
//! reviewer. A part Orchester still cannot read to the end goes to the reviewer too when its program only reads
//! (`grep -R`, `xargs grep`), with every path it names placed; what writes, redirects into a file, edits in place,
//! reaches the network or reads a credential still holds. Reads of the packages other managers install
//! (`node_modules`, `site-packages`, Go's module cache) are reads of dependency sources, not of credentials.
//! `cargo test -p orchester-core --test auto_mode_kontur kontur`.

use std::path::{Path, PathBuf};

use orchester_core::{
  permission::{
    AgentPermissions, CommandPermission, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
  },
  run::{
    ApprovalSubject, AutoVerdict, auto_verdict,
    review::{HoldRules, PushFacts, ReviewHold, review_hold, unread_parts},
  },
};
use tempfile::TempDir;

const REGISTRY: &str = ".cargo/registry/src/index.crates.io-1949cf8c6b5b557f";

/// A home folder with Kontur's worktree, the lurq crates Kontur's agents read in Cargo's registry, `.ssh`, and
/// packages of npm, Python and Go; the host's environment has `USERPROFILE` and, but on Windows, `HOME`.
struct Fixture {
  _root: TempDir,
  home: PathBuf,
  scope: CommandScope,
}

impl Fixture {
  fn new() -> Self {
    let root = tempfile::tempdir().expect("temporary folder");
    let home = root.path().join("home");
    let worktree = home.join("projects").join("kontur");
    let registry = home.join(REGISTRY);
    for folder in [
      worktree.join("src"),
      registry.join("lurq-0.51.1/src/components"),
      registry.join("lurq-0.51.1/src/render"),
      registry.join("lurq-0.51.1/src/app"),
      registry.join("lurq-0.51.1/examples"),
      registry.join("lurq-0.49.0/src/components"),
      registry.join("lurq-0.49.0/src/node"),
      registry.join("lurq-0.49.0/src/app"),
      registry.join("lurq-0.49.0/examples"),
      home.join(".ssh"),
      home.join("AppData/Roaming/npm/node_modules/typescript/lib"),
      home.join(".local/lib/python3.12/site-packages/requests"),
      home.join("go/pkg/mod/github.com/spf13/cobra@v1.8.0"),
      root.path().join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    for file in [
      worktree.join("src/lib.rs"),
      registry.join("lurq-0.51.1/Cargo.toml"),
      registry.join("lurq-0.49.0/Cargo.toml"),
      registry.join("lurq-0.49.0/README.md"),
      home.join(".ssh/id_rsa"),
      home.join("AppData/Roaming/npm/node_modules/typescript/lib/tsc.js"),
      home.join("AppData/Roaming/npm/node_modules/.npmrc"),
      home.join("AppData/Roaming/npm/node_modules/.yarnrc.yml"),
      home.join(".local/lib/python3.12/site-packages/requests/api.py"),
      home.join("go/pkg/mod/github.com/spf13/cobra@v1.8.0/command.go"),
    ] {
      std::fs::write(file, "x").expect("file");
    }
    let host = |name: &str| match name {
      "USERPROFILE" => Some(home.clone()),
      "HOME" if !cfg!(windows) => Some(home.clone()),
      _ => None,
    };
    let scope = CommandScope::in_environment(&worktree, &[], &root.path().join("temp"), host);
    Self {
      _root: root,
      home,
      scope,
    }
  }

  fn command(&self, line: &str) -> ClassifiedCommand {
    ClassifiedCommand::classify(line, &self.scope)
  }

  /// The hard rule that keeps `line` from the reviewer in a step like Kontur's Design Planner: it only reads the
  /// repository, its network is off and every command asks.
  fn hold(&self, line: &str) -> Option<ReviewHold> {
    let permissions = AgentPermissions::new(
      RepositoryAccess::ReadOnly,
      CommandPermission::AskBefore,
      NetworkAccess::Off,
    );
    let rules = HoldRules {
      permissions: Some(&permissions),
      push: PushFacts::default(),
      escalation: false,
      build_output: &[],
    };
    review_hold(&ApprovalSubject::Command(self.command(line)), &rules)
  }
}

/// The lines Kontur's Design Planner, Planner and Design Reviewer ran, as they ran them.
const KONTUR_READS: [&str; 10] = [
  r#"L=~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.51.1/src; ls $L/components $L/app; grep -rniE "fn with_decorations|decorations|drag_window|set_minimized|set_maximized|minimize|maximize|min_size|with_min|focused|WindowEvent::Focused|menu_bar|native menu|muda|blur|backdrop" $L --include=*.rs -l | head -40"#,
  r#"R=~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.51.1; ls $R/src/components $R/src/render; grep -rniE "fn (box_shadow|shadow|blur|backdrop|open_url|open_browser|clip)\b|backdrop|open::|webbrowser" $R/src --include=*.rs | head -30; grep -n "^\[dependencies" -A40 $R/Cargo.toml | grep -iE "open|webbrowser|arboard|clipboard""#,
  "ls -d ~/.cargo/registry/src/*/lurq-0.51* 2>/dev/null; ls ~/.cargo/registry/src/*/lurq-0.51*/src 2>/dev/null | head -80",
  "ls -d ~/.cargo/registry/src/*/lurq-0.51* && ls ~/.cargo/registry/src/*/lurq-0.51*/ ~/.cargo/registry/src/*/lurq-0.51*/src ~/.cargo/registry/src/*/lurq-0.51*/examples 2>/dev/null | head -150",
  r#"D=~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.49.0/src; grep -n "pub enum Dimension" -A12 $D/node/dimension.rs; grep -n "impl From<.*> for Dimension" $D/node/dimension.rs; grep -n "pub fn enable_mcp\|pub fn with_title\|pub fn id(" -r $D/app | head"#,
  r#"ls ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.49.0/src/components; grep -rn "pub fn \(background\|fill\|width\|height\|flex\|grow\|padding\|size_full\|fill_width\)" ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.49.0/src/components/column.rs ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.49.0/src/components/mod.rs 2>/dev/null | head -40"#,
  "ls -d ~/.cargo/registry/src/*/lurq-0.49.0 && ls ~/.cargo/registry/src/*/lurq-0.49.0 ~/.cargo/registry/src/*/lurq-0.49.0/examples",
  r#"cd ~/.cargo/registry/src/*/lurq-0.49.0 && ls; ls examples | head -20; sed -n 1,80p README.md; grep -n "rust-version\|edition\|^default" Cargo.toml"#,
  "ls -l --time-style=full-iso src/lib.rs",
  "cat .gitignore | head -20; cargo --version; rustc --version",
];

#[test]
fn kontur_read_only_lines_are_read_to_the_end_and_go_to_the_reviewer() {
  let fixture = Fixture::new();
  for line in KONTUR_READS {
    let command = fixture.command(line);
    assert_eq!(
      command.category(),
      CommandCategory::ReadOnly,
      "`{line}`: {:#?}",
      command.parts()
    );
    assert!(!command.reads_secrets(), "`{line}` reads no credential");
    assert_eq!(fixture.hold(line), None, "`{line}` goes to the reviewer");
  }
}

/// `~` and `$HOME` are the profile where Windows' environment has no `HOME`, as Git Bash sets it; elsewhere a shell
/// without `HOME` leaves them unread.
#[test]
fn kontur_home_is_the_profile_where_windows_sets_none() {
  let fixture = Fixture::new();
  for line in [
    "ls ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.51.1/src",
    "ls $HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.51.1/src",
  ] {
    assert_eq!(fixture.command(line).category(), CommandCategory::ReadOnly, "`{line}`");
  }
  assert!(fixture.command("cat ~/.ssh/id_rsa").reads_secrets());
  let worktree = fixture.home.join("projects").join("kontur");
  let unset = CommandScope::in_environment(&worktree, &[], &fixture.home, |_| None);
  assert_eq!(
    ClassifiedCommand::classify("ls ~/projects", &unset).category(),
    CommandCategory::Unknown,
    "no home folder at all"
  );
}

/// What Orchester cannot read to the end of a program that only reads goes to the reviewer, which is told it was not
/// read; auto mode never approves it by itself.
#[test]
fn kontur_unread_parts_that_only_read_go_to_the_reviewer() {
  let fixture = Fixture::new();
  for line in [
    "find src -name '*.rs' | xargs grep -n Placement",
    "find src -name '*.rs' -print0 | xargs -0 -n 4 grep -l Placement",
    "grep -R Placement src | head",
    "ls --hyperlink=auto src",
    "ls --hyperlink=auto ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lurq-0.51.1/src",
  ] {
    let command = fixture.command(line);
    assert_eq!(command.category(), CommandCategory::Unknown, "`{line}`");
    assert!(
      command
        .parts()
        .iter()
        .all(|part| part.only_reads() || part.category() == CommandCategory::ReadOnly)
    );
    let subject = ApprovalSubject::Command(command);
    assert!(unread_parts(&subject) > 0, "`{line}`: the reviewer is told");
    assert!(matches!(auto_verdict(&subject), AutoVerdict::Ask(_)), "`{line}`");
    assert_eq!(fixture.hold(line), None, "`{line}` goes to the reviewer");
  }
}

#[test]
fn kontur_writes_redirections_edits_and_the_network_still_hold() {
  let fixture = Fixture::new();
  let cases = [
    ("sed -i s/a/b/ src/lib.rs | head", ReviewHold::ReadOnlyStep),
    ("grep a src/lib.rs > found.txt", ReviewHold::ReadOnlyStep),
    ("ls --hyperlink=auto src > listing.txt", ReviewHold::Unreadable),
    ("grep -R a src >> ../found.txt", ReviewHold::Unreadable),
    ("find src -name '*.rs' | xargs rm", ReviewHold::Unreadable),
    ("find src | xargs -p grep a", ReviewHold::Unreadable),
    ("find src | xargs sed -i s/a/b/", ReviewHold::Unreadable),
    ("sort -o sorted.txt src/lib.rs", ReviewHold::Unreadable),
    ("D=$(curl https://example.dev/x); cat $D", ReviewHold::Unreadable),
    ("grep -n x $(cat list.txt)", ReviewHold::Unreadable),
    ("ls ~bob", ReviewHold::Unreadable),
    // A download may keep what it fetched; a lookup that keeps nothing still needs the network.
    ("curl https://example.dev | head", ReviewHold::ReadOnlyStep),
    (
      "cargo info lurq 2>&1 | head -40; ls ~/.cargo/registry/src/*/ | grep -i lurq",
      ReviewHold::ReadOnlyStep,
    ),
    ("cargo search lurq 2>&1 | head -5", ReviewHold::NetworkOff),
  ];
  for (line, hold) in cases {
    assert_eq!(
      fixture.hold(line),
      Some(hold),
      "`{line}`: {:#?}",
      fixture.command(line).parts()
    );
  }
}

/// A credential location stays held, also named by a part Orchester could not read to the end, and the rest of a
/// Cargo home stays one, as Kontur's Explorer met it.
#[test]
fn kontur_credential_reads_stay_held() {
  let fixture = Fixture::new();
  for line in [
    "cat ~/.ssh/id_rsa | head -3",
    "grep -R key ~/.ssh",
    "echo ~/.ssh/id_rsa | xargs cat",
    "ls --hyperlink=auto ~/.ssh",
    "cat .gitignore | head -20; cargo --version; rustc --version; ls ~/.cargo/registry 2>&1 | head",
    "cd ~/.cargo/registry/src/*/lurq-0.49.0 && sed -n 12,40p Cargo.toml; ls ~/.cargo/bin; rustup toolchain list",
    "cat ~/AppData/Roaming/npm/node_modules/.npmrc",
  ] {
    assert_eq!(fixture.hold(line), Some(ReviewHold::Secrets), "`{line}`");
  }
}

/// A reading cmdlet given a path on another provider, a comma array or a parameter that sets a variable is no read
/// Orchester can place: it stays held, in PowerShell's names and its aliases.
#[test]
fn kontur_powershell_provider_reads_stay_held() {
  let fixture = Fixture::new();
  for line in [
    "pwsh -NoProfile -Command 'Get-Content env:GH_TOKEN'",
    "pwsh -NoProfile -Command 'cat env:GH_TOKEN'",
    "pwsh -NoProfile -Command 'Get-ChildItem env:'",
    "pwsh -NoProfile -Command 'ls Variable:'",
    r"pwsh -NoProfile -Command 'Get-ChildItem Cert:\CurrentUser\My'",
    "pwsh -NoProfile -Command 'Select-String token -Path Env:PATH'",
    "pwsh -NoProfile -Command 'Get-Content src/lib.rs,env:GH_TOKEN'",
    "pwsh -NoProfile -Command 'Get-Content src/lib.rs -OutVariable PATH'",
  ] {
    let hold = fixture.hold(line);
    assert!(
      matches!(hold, Some(ReviewHold::Unreadable | ReviewHold::Secrets)),
      "`{line}` stays held: {hold:?} {:#?}",
      fixture.command(line).parts()
    );
  }
  assert_eq!(
    fixture.hold("pwsh -NoProfile -Command 'Get-Content src/lib.rs'"),
    None,
    "a file in the worktree is read"
  );
}

/// The packages npm, Python and Go install are read like Cargo's dependency sources, also among the home folder's
/// credential locations.
#[test]
fn kontur_package_sources_are_safe_reads() {
  let fixture = Fixture::new();
  let slashed = |path: &Path| path.to_string_lossy().replace('\\', "/");
  for line in [
    "cat ~/AppData/Roaming/npm/node_modules/typescript/lib/tsc.js | head".to_owned(),
    "grep -rn get ~/.local/lib/python3.12/site-packages/requests".to_owned(),
    "ls ~/go/pkg/mod/github.com/spf13/cobra@v1.8.0".to_owned(),
    format!(
      "wc -l {}/go/pkg/mod/github.com/spf13/cobra@v1.8.0/command.go",
      slashed(&fixture.home)
    ),
  ] {
    let command = fixture.command(&line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "`{line}`");
    assert!(!command.reads_secrets(), "`{line}`");
    assert!(
      command.parts().iter().all(|part| !part.reaches_outside()),
      "`{line}` reads like the worktree"
    );
    assert_eq!(fixture.hold(&line), None, "`{line}`");
  }
  for package in [
    "AppData/Roaming/npm/node_modules/typescript/lib/tsc.js",
    ".local/lib/python3.12/site-packages/requests/api.py",
  ] {
    assert!(
      fixture.scope.in_dependency_sources(&fixture.home.join(package)),
      "{package}"
    );
  }
  for token in [
    "AppData/Roaming/npm/node_modules/.npmrc",
    "AppData/Roaming/npm/node_modules/.yarnrc.yml",
  ] {
    assert!(
      !fixture.scope.in_dependency_sources(&fixture.home.join(token)),
      "{token}"
    );
  }
  assert!(!fixture.scope.in_dependency_sources(&fixture.home.join(".ssh/id_rsa")));
}
