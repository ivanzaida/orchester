//! Requests of a Claude Code agent that reached the person although nothing in them was theirs to decide (ORC-627,
//! seen on 2026-11-02 on macOS with Approve automatically on), by their exact command lines: reading its own task
//! output, the code-signing queries a prefix allows, `git log -L` on a file of the worktree and listing rustup's
//! targets. Each is read to the end, placed in the run's roots, kept within the step's limits, and answered by
//! Autopilot, the step's allowed commands and Approve automatically. A `pencil` read of the main checkout's design
//! (ORC-698) is placed by its file and exports.
//! `cargo test -p orchester-core --test approval_agent_requests approval_agent_requests`.

use std::path::{Path, PathBuf};

use orchester_core::{
  connection::{AgentProvider, invocation::Platform},
  id::{InvocationId, RunId, StepExecutionId},
  permission::{
    AgentPermissions, AllowedCommands, CommandPermission, CommandPrefix, NetworkAccess, RepositoryAccess,
    category::{ClassifiedCommand, CommandCategory, CommandScope},
    roots::{AgentHomes, RootsCheck, RunRoots},
  },
  run::{
    AnswerKind, ApprovalSubject, AutoAsk, AutoVerdict, ModeStep, PendingToolApproval, RequestAnswer, ReviewQuestion,
    RuleScope, auto_verdict, autopilot_check, autopilot_step, permitted,
    review::{StepLimit, step_limit},
  },
  time::Timestamp,
};
use serde_json::{Value, json};
use tempfile::TempDir;

const BRANCH: &str = "orchester/orc-627-run-1";
const USER: u32 = 501;

/// A main checkout in a folder whose name holds a space, as `/Volumes/External SSD/…` does, with the run's worktree
/// and its storage in it and `crates/desktop-app/Cargo.toml` there; a Builder that edits, with its network off and
/// `security find-identity` among its allowed commands.
struct Fixture {
  _root: TempDir,
  main: PathBuf,
  worktree: PathBuf,
  storage: PathBuf,
  elsewhere: PathBuf,
  roots: RunRoots,
  scope: CommandScope,
  permissions: AgentPermissions,
}

impl Fixture {
  fn new() -> Self {
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = std::fs::canonicalize(dir.path()).expect("canonical");
    let root = PathBuf::from(root.to_string_lossy().trim_start_matches(r"\\?\"));
    let main = root.join("External SSD").join("orchester");
    let worktree = main.join(".orchester").join("worktrees").join("w1");
    let storage = main.join(".orchester").join("runs").join("r1");
    let elsewhere = root.join("elsewhere");
    for folder in [
      &worktree.join("crates").join("desktop-app"),
      &storage,
      &elsewhere,
      &root.join("temp"),
    ] {
      std::fs::create_dir_all(folder).expect("folder");
    }
    std::fs::write(worktree.join("crates").join("desktop-app").join("Cargo.toml"), "").expect("file");
    let roots = RunRoots::new(Platform::current(), &worktree, BRANCH)
      .expect("worktree")
      .with_main_checkout(&main)
      .expect("main checkout")
      .with_run_storage(&storage)
      .expect("run storage")
      .with_agent_temp(
        AgentProvider::ClaudeCodeCli,
        AgentHomes {
          temp: &root.join("temp"),
          config: &root.join("config"),
          user: Some(USER),
        },
      );
    let scope =
      CommandScope::new(&worktree, &[], Some(&root.join("home")), Some(&root.join("temp"))).with_main_checkout(&main);
    let allowed = AllowedCommands::new(vec![CommandPrefix::new("security find-identity").expect("prefix")])
      .expect("allowed commands");
    let permissions = AgentPermissions::new(
      RepositoryAccess::Write,
      CommandPermission::Allowed(allowed),
      NetworkAccess::Off,
    );
    Self {
      _root: dir,
      main,
      worktree,
      storage,
      elsewhere,
      roots,
      scope,
      permissions,
    }
  }

  fn command(&self, line: &str) -> ClassifiedCommand {
    ClassifiedCommand::classify(line, &self.scope)
  }

  fn subject(&self, line: &str) -> ApprovalSubject {
    ApprovalSubject::Command(self.command(line))
  }

  /// What Autopilot does with `line`: by the roots, within the step's limits.
  fn autopilot(&self, line: &str) -> ModeStep {
    let subject = self.subject(line);
    let limit = step_limit(&subject, Some(&self.permissions), &[]);
    autopilot_step(&autopilot_check(&self.roots, &subject), limit)
  }

  /// `line` is read to the end as a read that installs nothing and reads no credential, Autopilot approves it by its
  /// rule, and Approve automatically may approve it.
  fn assert_answered(&self, line: &str) {
    let command = self.command(line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "{line}");
    assert!(!command.reads_secrets() && !command.installs(), "{line}");
    assert_eq!(
      self.autopilot(line),
      ModeStep::Approve(RequestAnswer::by(AnswerKind::Autopilot)),
      "{line}"
    );
    assert_eq!(
      auto_verdict(&self.subject(line)),
      AutoVerdict::Approve(Some(CommandCategory::ReadOnly)),
      "{line}"
    );
  }
}

fn slashed(path: &Path) -> String {
  path.to_string_lossy().replace('\\', "/")
}

/// Claude Code's background commands write their output to `/tmp/claude-<uid>/<worktree slug>/<session>/tasks/`
/// on macOS, where its `tail` reads it: the slug is the worktree with every character but a letter or digit, a
/// space too, as `-`.
#[cfg(target_os = "macos")]
#[test]
fn approval_agent_requests_reads_its_own_task_output() {
  let fixture = Fixture::new();
  let slug: String = fixture
    .worktree
    .to_string_lossy()
    .chars()
    .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
    .collect();
  assert!(slug.contains("-External-SSD-"), "{slug}");
  let output = format!("/private/tmp/claude-{USER}/{slug}/3f0c2a1e-session/tasks/bq5x2k.output");
  fixture.assert_answered(&format!("tail -n 40 {output}"));
  assert_eq!(
    fixture
      .roots
      .check(&orchester_core::permission::roots::ToolRequest::Files(&[
        output.replacen("/private", "", 1)
      ])),
    RootsCheck::Inside
  );
  // Another worktree's task output is not the run's.
  let other = format!("/private/tmp/claude-{USER}/-Users-ana-other/s/tasks/b.output");
  assert!(matches!(
    autopilot_check(&fixture.roots, &fixture.subject(&format!("tail -n 40 {other}"))),
    RootsCheck::Outside { .. }
  ));
}

/// `security find-identity` lists the signing identities, no private key: a prefix of it in the allowed commands
/// runs it. `spctl --assess` reads the files it names, which are placed. The rest of `security` still reads a
/// credential store, and the rest of `spctl` changes the system's policy.
#[test]
fn approval_agent_requests_code_signing_queries_are_read_to_the_end() {
  let fixture = Fixture::new();
  let identities = "security find-identity -v -p codesigning";
  fixture.assert_answered(identities);
  assert_eq!(
    permitted(&fixture.subject(identities), &fixture.permissions),
    Some(CommandCategory::ReadOnly)
  );
  let assess = fixture.command("spctl -a -vv -t exec /Volumes/Orchester/Orchester.app");
  assert_eq!(assess.category(), CommandCategory::ReadOnly);
  assert!(!assess.reads_secrets());
  // The app it assesses is placed: outside the roots it comes to the person as such.
  let app = format!("{}/Orchester.app", slashed(&fixture.elsewhere));
  assert!(matches!(
    autopilot_check(&fixture.roots, &fixture.subject(&format!("spctl -a -vv -t exec {app}"))),
    RootsCheck::Outside { .. }
  ));
  std::fs::create_dir_all(fixture.worktree.join("target").join("Orchester.app")).expect("folder");
  fixture.assert_answered("spctl -a -vv -t exec target/Orchester.app");
  fixture.assert_answered("spctl --assess --verbose --type exec target/Orchester.app");

  for line in ["security dump-keychain", "security find-generic-password -s x -w"] {
    let command = fixture.command(line);
    assert!(command.reads_secrets(), "{line}");
    assert_eq!(command.category(), CommandCategory::Unknown, "{line}");
    assert_eq!(permitted(&fixture.subject(line), &fixture.permissions), None, "{line}");
  }
  for line in [
    "security find-identity -v -x codesigning",
    "spctl --master-disable",
    "spctl --add --label x target/Orchester.app",
    "spctl -a",
  ] {
    assert_eq!(fixture.command(line).category(), CommandCategory::Unknown, "{line}");
  }
}

/// The range of `git log -L` is no path: only its file, after the last colon, is placed.
#[test]
fn approval_agent_requests_git_log_line_range_is_no_path() {
  let fixture = Fixture::new();
  fixture.assert_answered(
    r#"git log --format="%h %ad %s" --date=iso -L "/^version/,+1:crates/desktop-app/Cargo.toml" --no-patch"#,
  );
  fixture.assert_answered("git log -L/^version/,+1:crates/desktop-app/Cargo.toml --no-patch");
  fixture.assert_answered("git log -L :main:crates/desktop-app/Cargo.toml -- crates");
  let outside = slashed(&fixture.elsewhere.join("Cargo.toml"));
  let check = autopilot_check(
    &fixture.roots,
    &fixture.subject(&format!("git log -L \"1,2:{outside}\" --no-patch")),
  );
  assert!(
    matches!(&check, RootsCheck::Outside { path } if path.ends_with("elsewhere/Cargo.toml")),
    "{check:?}"
  );
}

/// Listing or querying a toolchain or packages installs nothing; adding a target does.
#[test]
fn approval_agent_requests_listing_installs_nothing() {
  let fixture = Fixture::new();
  for line in [
    "rustup target list --installed",
    "rustup target list",
    "rustup toolchain list",
    "rustup component list --installed",
    "rustup show",
    "cargo --version",
    "brew list",
  ] {
    fixture.assert_answered(line);
    assert_eq!(
      step_limit(&fixture.subject(line), Some(&fixture.permissions), &[]),
      StepLimit::Within,
      "{line}"
    );
  }
  for line in [
    "rustup target add aarch64-apple-darwin",
    "rustup toolchain install nightly",
  ] {
    assert!(fixture.command(line).installs(), "{line}");
  }
}

/// Permissions with `repository` access and `prefixes` as the allowed commands, the network off.
fn allowing(repository: RepositoryAccess, prefixes: &[&str]) -> AgentPermissions {
  let prefixes = prefixes
    .iter()
    .map(|prefix| CommandPrefix::new(prefix).expect("prefix"))
    .collect();
  AgentPermissions::new(
    repository,
    CommandPermission::Allowed(AllowedCommands::new(prefixes).expect("allowed commands")),
    NetworkAccess::Off,
  )
}

/// The repository's own check, which a Review step asked to run in ORC-658 and ORC-647, was read as a script
/// Orchester cannot read to the end (ORC-662): `pwsh -File` without `-NoProfile` runs the person's own profile
/// first, not code from elsewhere. A script of the worktree's `tools/` runs as the step's allowed commands judge
/// it, `pwsh` listed or the step editing; one outside the worktree is not read.
#[test]
fn approval_agent_requests_repository_check_script_runs() {
  let fixture = Fixture::new();
  let tools = fixture.worktree.join("tools");
  std::fs::create_dir_all(&tools).expect("folder");
  for script in ["check.ps1", "check.sh"] {
    std::fs::write(tools.join(script), "").expect("file");
    std::fs::write(fixture.elsewhere.join(script), "").expect("file");
  }
  let builder = allowing(RepositoryAccess::Write, &["pwsh", "tools/"]);
  let editing = allowing(RepositoryAccess::Write, &["tools/"]);
  let reviewer = allowing(RepositoryAccess::ReadOnly, &["pwsh"]);
  for line in ["pwsh -File tools/check.ps1", "pwsh -NoProfile -File tools/check.ps1"] {
    let subject = fixture.subject(line);
    assert_eq!(fixture.command(line).category(), CommandCategory::Worktree, "{line}");
    assert_eq!(
      auto_verdict(&subject),
      AutoVerdict::Approve(Some(CommandCategory::Worktree)),
      "{line}"
    );
    for permissions in [&builder, &editing, &reviewer] {
      assert_eq!(
        permitted(&subject, permissions),
        Some(CommandCategory::Worktree),
        "{line} with {permissions:?}"
      );
    }
    // In Autopilot too, with `pwsh` listed; a step that lists no program for it leaves it to the reviewer.
    for permissions in [&builder, &reviewer] {
      let limit = step_limit(&subject, Some(permissions), &[]);
      assert_eq!(limit, StepLimit::Within, "{line} with {permissions:?}");
      assert_eq!(
        autopilot_step(&autopilot_check(&fixture.roots, &subject), limit),
        ModeStep::Approve(RequestAnswer::by(AnswerKind::Autopilot)),
        "{line}"
      );
    }
    assert_eq!(step_limit(&subject, Some(&editing), &[]), StepLimit::Unread, "{line}");
  }
  let bash = allowing(RepositoryAccess::ReadOnly, &["bash", "sh"]);
  for line in ["bash tools/check.sh", "sh tools/check.sh"] {
    let subject = fixture.subject(line);
    assert_eq!(fixture.command(line).category(), CommandCategory::Worktree, "{line}");
    assert_eq!(permitted(&subject, &editing), Some(CommandCategory::Worktree), "{line}");
    assert_eq!(permitted(&subject, &bash), Some(CommandCategory::Worktree), "{line}");
  }

  // A script outside the worktree, absolute or relative, is code from elsewhere.
  let outside = slashed(&fixture.elsewhere);
  for line in [
    format!("pwsh -File {outside}/check.ps1"),
    format!("pwsh -NoProfile -File {outside}/check.ps1"),
    "pwsh -File ../../../../../elsewhere/check.ps1".to_owned(),
    format!("bash {outside}/check.sh"),
  ] {
    let subject = fixture.subject(&line);
    assert_eq!(fixture.command(&line).category(), CommandCategory::Unknown, "{line}");
    assert_eq!(auto_verdict(&subject), AutoVerdict::Ask(AutoAsk::Unknown), "{line}");
    assert_eq!(permitted(&subject, &builder), None, "{line}");
    assert_eq!(step_limit(&subject, Some(&builder), &[]), StepLimit::Unread, "{line}");
  }
}

/// ORC-280's Builder read the project's main checkout with git on 2026-11-04 and automation could not approve it
/// (ORC-695): git anywhere but the worktree loads that folder's settings and code, but the run's worktree is a
/// worktree of the main checkout's repository and loads the same `.git` settings and hooks. Git that only reads
/// there is a read outside the worktree: Autopilot asks, as it touches the main checkout, and Approve automatically
/// may approve it. Its global options, options and subcommand name no path there; an option's value still may.
#[test]
fn approval_agent_requests_git_reads_main_checkout() {
  let fixture = Fixture::new();
  let main = slashed(&fixture.main);
  for (line, values) in [
    (
      format!(
        r#"git -C "{main}" status --porcelain=v2 --branch --no-renames -z --untracked-files=all --ignored=matching"#
      ),
      &["v2", "all", "matching"][..],
    ),
    (format!(r#"git -C "{main}" ls-files --stage -z"#), &[]),
    (format!(r#"git -C "{main}" log --oneline -5"#), &[]),
    ("git -C ../../.. rev-parse HEAD".to_owned(), &["HEAD"]),
  ] {
    let command = fixture.command(&line);
    assert_eq!(command.category(), CommandCategory::ReadOnly, "{line}");
    let part = &command.parts()[0];
    assert!(part.reaches_outside() && !part.writes_outside(), "{line}");
    let touches = part.touches();
    assert!(touches.contains(&fixture.main), "{line}: {touches:?}");
    assert!(
      touches.iter().all(|path| *path == fixture.main
        || *path == fixture.worktree
        || values.iter().any(|value| *path == fixture.main.join(value))),
      "{line}: {touches:?}"
    );
    assert!(
      matches!(
        autopilot_check(&fixture.roots, &fixture.subject(&line)),
        RootsCheck::MainCheckout { .. }
      ),
      "{line}"
    );
    assert_ne!(
      fixture.autopilot(&line),
      ModeStep::Approve(RequestAnswer::by(AnswerKind::Autopilot)),
      "{line}"
    );
    assert_eq!(
      auto_verdict(&fixture.subject(&line)),
      AutoVerdict::Approve(Some(CommandCategory::ReadOnly)),
      "{line}"
    );
  }
  // As before the fix, without a `-C`.
  for line in [
    "git status --porcelain=v2 --branch --no-renames -z --untracked-files=all --ignored=matching",
    "git ls-files --stage -z",
  ] {
    fixture.assert_answered(line);
  }
  // Another repository loads its own settings and code.
  let elsewhere = slashed(&fixture.elsewhere);
  for line in [
    format!("git -C {elsewhere} status"),
    format!(r#"git -C "{main}/crates" status"#),
    format!(r#"git -C "{main}" -C crates status"#),
  ] {
    let subject = fixture.subject(&line);
    assert_eq!(fixture.command(&line).category(), CommandCategory::Unknown, "{line}");
    assert_eq!(auto_verdict(&subject), AutoVerdict::Ask(AutoAsk::Unknown), "{line}");
  }
  // A write in the main checkout is judged as before: a write outside the worktree, or Unknown.
  for (line, category) in [
    (format!(r#"git -C "{main}" add -A"#), CommandCategory::Unknown),
    (format!(r#"git -C "{main}" commit -m x"#), CommandCategory::Unknown),
    (
      format!(r#"git -C "{main}" reset --hard HEAD"#),
      CommandCategory::HistoryDestructive,
    ),
  ] {
    let command = fixture.command(&line);
    assert_eq!(command.category(), category, "{line}");
    assert!(command.parts()[0].writes_outside(), "{line}");
  }
}

/// A call of the `pencil` server's `tool` with `arguments`, as Claude Code's prompt records it: `filePath` as its
/// target, the arguments as JSON.
fn pencil_call(fixture: &Fixture, tool: &str, arguments: &Value) -> ApprovalSubject {
  let approval = PendingToolApproval {
    id: "invocation:1".to_owned(),
    run: RunId::generate(),
    execution: StepExecutionId::generate(),
    invocation: InvocationId::generate(),
    agent: "Builder".to_owned(),
    command: false,
    scope: Some(arguments.to_string()),
    server: "pencil".to_owned(),
    action: tool.to_owned(),
    target: arguments["filePath"].as_str().map(str::to_owned),
    requested_at: Timestamp::from_unix_millis(0),
    native: None,
  };
  ApprovalSubject::of(&approval, &fixture.scope)
}

/// What Autopilot does with `subject`, within the step's limits.
fn autopilot_on(fixture: &Fixture, subject: &ApprovalSubject) -> ModeStep {
  let limit = step_limit(subject, Some(&fixture.permissions), &[]);
  autopilot_step(&autopilot_check(&fixture.roots, subject), limit)
}

/// The main checkout's design, as `AGENTS.md` asks a step to name it.
fn main_design(fixture: &Fixture) -> String {
  slashed(&fixture.main.join("crates/desktop-app/design/orchester.pen"))
}

/// A `pencil` `execute` of `input` on the main checkout's design.
fn execute(fixture: &Fixture, input: &str) -> ApprovalSubject {
  pencil_call(
    fixture,
    "execute",
    &json!({"filePath": main_design(fixture), "input": input}),
  )
}

/// UI runs read the design through `pencil` `execute` on the main checkout's `orchester.pen`, as `AGENTS.md` asks,
/// and every call came to the person as `reviewerUnsure` (ORC-698, seen in ORC-362, ORC-363, ORC-364 and ORC-697). A
/// snippet that only reads, exporting at most into the worktree or the run's storage, runs under Autopilot.
#[test]
fn approval_agent_requests_pencil_reads_main_checkout_design() {
  let fixture = Fixture::new();
  let storage = slashed(&fixture.storage.join("screenshots"));
  let tmp = slashed(&fixture.worktree.join(".tmp/orc-698"));
  let approved = ModeStep::Approve(RequestAnswer::by(AnswerKind::Autopilot));
  for input in [
    r#"Print(Get((n,c)=>{if(c.depth===0){c.skipChildren();return n.name}}).join(" | "))"#.to_owned(),
    r#"Print(Get("H6kPd",{depth:2}))"#.to_owned(),
    r#"TakeScreenshot(["H6kPd"])"#.to_owned(),
    r#"Get("H6kPd",(n,c)=>Print(n.id,n.name,c.bounds.width,`${c.depth}`))"#.to_owned(),
    r#"const kids=Get("H6kPd",{depth:1}).children; Print(kids[0].name, "Insert row", /Row \d+/.test(kids[0].name))"#
      .to_owned(),
    format!(r#"Export(["H6kPd","p6tXIO"],"png","{storage}")"#),
    format!(r#"TakeScreenshot(["H6kPd"]); Export(["H6kPd"], "png", "{tmp}", {{scale: 1}})"#),
  ] {
    let subject = execute(&fixture, &input);
    assert_eq!(autopilot_check(&fixture.roots, &subject), RootsCheck::Inside, "{input}");
    assert_eq!(autopilot_on(&fixture, &subject), approved, "{input}");
  }
  // Approve similar still covers the tool.
  assert_eq!(
    RuleScope::similar(&execute(&fixture, r#"TakeScreenshot(["H6kPd"])"#)),
    RuleScope::similar(&execute(&fixture, r#"Insert(document,{type:"frame"})"#))
  );
}

/// An export elsewhere, into the main checkout outside the run's storage, or by a relative path writes outside the
/// roots; so does another design file (ORC-698).
#[test]
fn approval_agent_requests_pencil_exports_elsewhere_ask() {
  let fixture = Fixture::new();
  let elsewhere = slashed(&fixture.elsewhere);
  let main = slashed(&fixture.main);
  for input in [
    format!(r#"Export(["H6kPd"],"png","{elsewhere}")"#),
    format!(r#"Export(["H6kPd"],"png","{main}/crates/desktop-app/design/exports")"#),
  ] {
    assert!(
      matches!(autopilot_on(&fixture, &execute(&fixture, &input)), ModeStep::Person(_)),
      "{input}"
    );
  }
  let relative = execute(&fixture, r#"Export(["H6kPd"],"png","./exports")"#);
  assert_eq!(
    autopilot_on(&fixture, &relative),
    ModeStep::Review(ReviewQuestion::Roots)
  );
  let other = json!({"filePath": format!("{elsewhere}/other.pen"), "input": r#"TakeScreenshot(["a"])"#});
  assert!(matches!(
    autopilot_on(&fixture, &pencil_call(&fixture, "execute", &other)),
    ModeStep::Person(_)
  ));
}

/// What may change the design, or reach a function by a name not written out, is not placed: it goes to the
/// reviewer's question as before (ORC-698).
#[test]
fn approval_agent_requests_pencil_writes_still_ask() {
  let fixture = Fixture::new();
  for input in [
    r#"Insert(document,{type:"frame",name:"x"})"#,
    r#"Update("H6kPd",{name:"x"})"#,
    r#"Get(n=>n.name==="Row"&&Delete(n.id))"#,
    r#"Copy("H6kPd",document)"#,
    r#"Replace("a/b",{type:"text"})"#,
    r#"Move("a",document)"#,
    r##"SetVariables({accent:{type:"color",value:"#fff"}})"##,
    r#"Generate("stock","a","cats")"#,
    r#"globalThis["Ins"+"ert"](document,{})"#,
    r#"eval("Insert(document,{})")"#,
    r#"[]["con"+"structor"]"#,
    r#"const k="x"; Print(Get("a")[k])"#,
    "\\u0049nsert(document,{})",
    r#"const e=Export; e(["a"],"png","/tmp")"#,
    r#"Export(["a"],"png",`/tmp/${1}`)"#,
    r#"Print("unterminated)"#,
  ] {
    let subject = execute(&fixture, input);
    assert_eq!(
      autopilot_check(&fixture.roots, &subject),
      RootsCheck::Unknown,
      "{input}"
    );
    assert_eq!(
      autopilot_on(&fixture, &subject),
      ModeStep::Review(ReviewQuestion::Roots),
      "{input}"
    );
  }
  let design = main_design(&fixture);
  let writing_patch = json!({"filePath": design, "editId": "e1", "edits": [{"find": "Get(", "replace": "Update("}]});
  let reading_patch = json!({"filePath": design, "editId": "e1", "edits": [{"find": "depth:1", "replace": "depth:2"}]});
  let no_file = json!({"input": r#"TakeScreenshot(["a"])"#});
  for (tool, arguments) in [
    ("execute", &writing_patch),
    ("execute", &reading_patch),
    ("execute", &no_file),
    ("get_screenshot", &json!({"filePath": design, "input": "Print(1)"})),
  ] {
    let subject = pencil_call(&fixture, tool, arguments);
    assert_eq!(
      autopilot_check(&fixture.roots, &subject),
      RootsCheck::Unknown,
      "{arguments}"
    );
  }
}

/// Pencil runs the whole patched snippet, so a patch whose every piece reads only can still move a quote and make
/// the failed snippet's string into code: an `edits` patch goes to the reviewer (ORC-698, found in review).
#[test]
fn approval_agent_requests_pencil_patch_goes_to_reviewer() {
  let fixture = Fixture::new();
  // `Insert` sits in a string: the snippet reads only, and fails at `.a.b`.
  let failed = execute(&fixture, r#"Print(" Insert(document,{}); Print(");Get("missing").a.b"#);
  assert_eq!(autopilot_check(&fixture.roots, &failed), RootsCheck::Inside);
  // Patched, it runs `Print("");Insert(document,{}); Print("")`.
  let patch = json!({
    "filePath": main_design(&fixture),
    "editId": "e1",
    "edits": [
      {"find": r#"Print(" "#, "replace": r#"Print("");"#},
      {"find": r#"Print(");Get("missing").a.b"#, "replace": r#"Print("")"#},
    ],
  });
  let patched = pencil_call(&fixture, "execute", &patch);
  assert_eq!(autopilot_check(&fixture.roots, &patched), RootsCheck::Unknown);
  assert_eq!(
    autopilot_on(&fixture, &patched),
    ModeStep::Review(ReviewQuestion::Roots)
  );
}
