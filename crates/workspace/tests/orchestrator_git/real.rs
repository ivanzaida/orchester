//! The real programs (`Programs::processes`), not a test runner: a local write stopped at its timeout over a slow hook,
//! a read cut while git writes it (`run_capped`), and network git run with the ssh command that never asks. The
//! repository's own settings name its author and hooks folder, so the person's global ones change nothing that is
//! checked here; the test process must not set `GIT_SSH_COMMAND` or `GIT_SSH`.

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::{Duration, Instant},
};

use orchester_core::{
  id::ProjectId,
  name::Name,
  project::{Project, ProjectKey},
};
use orchester_exec::programs::Programs;
use orchester_store::{ProjectDb, exclude_project_data};
use orchester_workspace::orchestrator::git::{Called, Timeouts, call};
use serde_json::{Value, json};
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
  let output = Command::new("git")
    .args(args)
    .current_dir(dir)
    .output()
    .expect("git runs");
  assert!(
    output.status.success(),
    "git {args:?}: {}",
    String::from_utf8_lossy(&output.stderr)
  );
}

/// A project of its own, its author and hooks folder in its own settings.
fn project() -> (TempDir, PathBuf) {
  let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/orchestrator-git/real");
  fs::create_dir_all(&scratch).expect("a scratch folder");
  let dir = tempfile::tempdir_in(scratch).expect("a folder of the test's own");
  let root = dir.path().join("kontur");
  fs::create_dir_all(&root).expect("the project's folder");
  git(&root, &["init", "-b", "main"]);
  for (key, value) in [
    ("user.name", "Test Author"),
    ("user.email", "author@example.com"),
    ("core.hooksPath", ".githooks"),
    ("core.autocrlf", "false"),
    ("commit.gpgsign", "false"),
  ] {
    git(&root, &["config", key, value]);
  }
  fs::write(root.join("README.md"), "Kontur\n").expect("a file");
  git(&root, &["add", "-A"]);
  git(&root, &["commit", "-m", "Initial commit"]);
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("ORC").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  ProjectDb::create(&root, project).expect("the project's data");
  exclude_project_data(&root).expect("kept out of git");
  (dir, root)
}

fn called(root: &Path, (name, arguments): (&str, Value), timeouts: Timeouts) -> Called {
  let Value::Object(arguments) = arguments else {
    panic!("arguments are an object");
  };
  call(&Programs::processes(), root, (name, &arguments), (true, timeouts)).expect("the tool ran")
}

fn refusal(called: Called) -> String {
  match called {
    Called::Wrote {
      result: Err(orchester_workspace::app_mcp::ToolError::Refused { code, .. }),
      ..
    } => code,
    other => panic!("a refusal: {other:?}"),
  }
}

#[test]
fn orchestrator_git_real_local_write_stops_at_its_timeout() {
  let (_dir, root) = project();
  let hooks = root.join(".githooks");
  fs::create_dir_all(&hooks).expect("the hooks folder");
  fs::write(hooks.join("pre-commit"), "#!/bin/sh\nsleep 30\n").expect("a slow hook");
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).expect("executable");
  }
  fs::write(root.join("notes.txt"), "notes\n").expect("a file");
  let short = Timeouts {
    local: Duration::from_secs(3),
    network: Duration::from_secs(3),
  };
  let started = Instant::now();
  let commit = called(
    &root,
    ("git_commit", json!({ "message": "Notes", "paths": ["notes.txt"] })),
    short,
  );
  assert_eq!(refusal(commit), "timed_out");
  assert!(
    started.elapsed() < Duration::from_secs(25),
    "stopped, not waited out: {:?}",
    started.elapsed()
  );
}

#[test]
fn orchestrator_git_real_reads_are_cut_while_git_writes() {
  let (_dir, root) = project();
  fs::write(root.join("big.txt"), "line of a big file\n".repeat(10_000)).expect("a big file");
  git(&root, &["add", "big.txt"]);
  git(&root, &["commit", "-m", "Big"]);
  let Value::Object(arguments) = json!({ "rev": "HEAD", "file": "big.txt" }) else {
    unreachable!("an object");
  };
  let read = call(
    &Programs::processes(),
    &root,
    ("git_show", &arguments),
    (false, Timeouts::STANDARD),
  );
  let Ok(Called::Read(read)) = read else {
    panic!("a read: {read:?}");
  };
  assert_eq!(read["truncated"], true);
  assert!(read["text"].as_str().expect("text").len() <= 60_000);
}

#[test]
fn orchestrator_git_real_network_git_runs_ssh_in_batch_mode() {
  assert!(
    std::env::var_os("GIT_SSH_COMMAND").is_none() && std::env::var_os("GIT_SSH").is_none(),
    "the test process sets no ssh of its own"
  );
  let (dir, root) = project();
  let said = dir.path().join("ssh-args.txt");
  let fake = dir.path().join("fake-ssh.sh");
  let forward = |path: &Path| path.to_string_lossy().replace('\\', "/");
  fs::write(
    &fake,
    format!("#!/bin/sh\necho \"$@\" > '{}'\nexit 255\n", forward(&said)),
  )
  .expect("a stand-in ssh");
  git(
    &root,
    &["config", "core.sshCommand", &format!("sh '{}'", forward(&fake))],
  );
  git(&root, &["remote", "add", "far", "ssh://git@example.invalid/kontur.git"]);
  let fetched = called(&root, ("git_fetch", json!({ "remote": "far" })), Timeouts::STANDARD);
  let code = refusal(fetched);
  assert!(code == "git_failed" || code == "auth_failed", "{code}");
  let args = fs::read_to_string(&said).expect("git ran the stand-in");
  assert!(args.contains("-o BatchMode=yes"), "{args}");
}
