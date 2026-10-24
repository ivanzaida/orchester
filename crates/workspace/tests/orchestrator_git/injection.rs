//! What the chat names never reaches git as more than a name: a branch is no refspec (`+a:b` would force-overwrite a
//! ref, `*` match every branch), a remote is one the repository lists (never a URL or a folder to send code to or take
//! history from), a start point no expression, and a path stays inside the repository. Each call goes through the
//! tools themselves and changes nothing.

use orchester_workspace::{app_mcp::ToolError, orchestrator::git::Called};
use serde_json::{Value, json};

use super::scratch::Scratch;

/// Kontur with `wip` one commit ahead of `victim`, both on `origin`, and a tag `t2`; the refs as they stand.
fn refs() -> (Scratch, String) {
  let scratch = Scratch::new();
  scratch.git(&["branch", "victim"]);
  scratch.git(&["switch", "-c", "wip"]);
  scratch.commit("wip.txt", "wip\n", "Work in progress");
  scratch.git(&["switch", "main"]);
  scratch.git(&["tag", "t2", "victim"]);
  scratch.git(&["push", "origin", "wip", "victim", "t2"]);
  let stand = refs_now(&scratch);
  (scratch, stand)
}

fn refs_now(scratch: &Scratch) -> String {
  scratch.git(&["for-each-ref", "--format=%(refname) %(objectname)"])
}

fn remote_refs(scratch: &Scratch) -> String {
  scratch.git_in(
    &scratch.origin(),
    &["for-each-ref", "--format=%(refname) %(objectname)"],
  )
}

/// Whether `called` was refused for its arguments, before git ran anything.
fn invalid(called: Result<Called, ToolError>) -> bool {
  match called {
    Err(ToolError::InvalidArguments(_)) => true,
    Ok(Called::Wrote {
      result: Err(ToolError::InvalidArguments(_)),
      change,
    }) => change.commands.is_empty(),
    _ => false,
  }
}

fn assert_invalid(scratch: &Scratch, name: &str, arguments: &Value) {
  let called = scratch.call(name, arguments, true);
  assert!(invalid(called), "{name} {arguments} is refused for its arguments");
}

#[test]
fn orchestrator_git_injection_refspecs_and_globs_never_reach_git() {
  let (scratch, stand) = refs();
  let origin_before = remote_refs(&scratch);
  for branch in [
    "+refs/heads/wip:refs/heads/victim",
    "+refs/heads/*:refs/heads/*",
    "+refs/heads/wip:refs/tags/t2",
    "wip:victim",
    "*",
    "wip~1",
    "@{-1}",
    "-f",
  ] {
    assert_invalid(&scratch, "git_pull", &json!({ "remote": "origin", "branch": branch }));
    assert_invalid(&scratch, "git_push", &json!({ "branch": branch }));
    assert_invalid(&scratch, "git_merge", &json!({ "branch": branch }));
    assert_invalid(&scratch, "git_switch", &json!({ "branch": branch }));
    assert_invalid(&scratch, "git_create_branch", &json!({ "name": branch }));
    assert_invalid(
      &scratch,
      "git_create_branch",
      &json!({ "name": "fresh", "start": branch }),
    );
    assert_invalid(&scratch, "git_tag", &json!({ "name": branch }));
    assert_invalid(&scratch, "git_tag", &json!({ "name": "fresh", "rev": branch }));
    for operation in [
      json!({ "kind": "delete_branch", "branch": branch }),
      json!({ "kind": "force_push", "branch": branch }),
      json!({ "kind": "switch_discarding", "branch": branch }),
      json!({ "kind": "delete_tag", "tag": branch }),
    ] {
      assert_invalid(&scratch, "git_propose", &json!({ "operation": operation }));
    }
  }
  assert_eq!(refs_now(&scratch), stand, "no ref moved");
  assert_eq!(remote_refs(&scratch), origin_before, "nothing was pushed");
}

#[test]
fn orchestrator_git_injection_remotes_are_only_the_listed_ones() {
  let (scratch, stand) = refs();
  let origin_before = remote_refs(&scratch);
  let elsewhere = scratch.dir.path().join("elsewhere.git");
  scratch.git_in(scratch.dir.path(), &["init", "--bare", "elsewhere.git"]);
  let remotes = [
    scratch.origin().to_string_lossy().into_owned(),
    elsewhere.to_string_lossy().into_owned(),
    "https://user:token@example.com/kontur.git".to_owned(),
    "git@example.com:kontur.git".to_owned(),
    "../origin.git".to_owned(),
    "upstream".to_owned(),
  ];
  for remote in &remotes {
    assert_invalid(&scratch, "git_push", &json!({ "remote": remote, "branch": "wip" }));
    assert_invalid(&scratch, "git_pull", &json!({ "remote": remote, "branch": "wip" }));
    assert_invalid(&scratch, "git_fetch", &json!({ "remote": remote }));
    for operation in [
      json!({ "kind": "force_push", "remote": remote, "branch": "wip" }),
      json!({ "kind": "delete_branch", "remote": remote, "branch": "wip" }),
      json!({ "kind": "delete_tag", "remote": remote, "tag": "t2" }),
    ] {
      assert_invalid(&scratch, "git_propose", &json!({ "operation": operation }));
    }
  }
  assert_eq!(refs_now(&scratch), stand, "nothing was taken in");
  assert!(
    scratch.git_in(&elsewhere, &["for-each-ref"]).is_empty(),
    "nothing was sent to a folder the chat named"
  );
  assert_eq!(remote_refs(&scratch), origin_before);
}

#[test]
fn orchestrator_git_injection_paths_stay_inside_the_repository() {
  let scratch = Scratch::new();
  std::fs::write(scratch.dir.path().join("outside.txt"), "outside\n").expect("a file beside the repository");
  scratch.write("README.md", "edited\n");
  let head = scratch.head("HEAD");
  for path in [
    "../outside.txt",
    "/etc/hosts",
    "C:/Windows/win.ini",
    "..",
    ":(top)README.md",
    "-p",
  ] {
    for (name, arguments) in [
      ("git_diff", json!({ "paths": [path] })),
      ("git_log", json!({ "path": path })),
      ("git_show", json!({ "rev": "HEAD", "file": path })),
      ("git_blame", json!({ "path": path })),
      ("git_commit", json!({ "message": "Out", "paths": [path] })),
    ] {
      assert_invalid(&scratch, name, &arguments);
    }
    for operation in [
      json!({ "kind": "clean", "paths": [path] }),
      json!({ "kind": "discard_changes", "paths": [path] }),
    ] {
      assert_invalid(&scratch, "git_propose", &json!({ "operation": operation }));
    }
  }
  assert_eq!(scratch.head("HEAD"), head);
  assert_eq!(scratch.read("README.md"), "edited\n");
}
