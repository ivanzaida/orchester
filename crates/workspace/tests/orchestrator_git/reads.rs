//! Every read tool answers from the repository and changes nothing, with the person's switch off.

use serde_json::json;

use super::scratch::Scratch;

/// Kontur with a commit not pushed, an edit and a new file: the reads' repository, and its HEAD.
fn edited() -> (Scratch, String) {
  let scratch = Scratch::new();
  scratch.commit("src/lib.rs", "pub fn one() {}\n", "Add one");
  scratch.write("README.md", "Kontur, edited\n");
  scratch.write("notes.txt", "new\n");
  let head = scratch.head("HEAD");
  (scratch, head)
}

#[test]
fn orchestrator_git_reads_status() {
  let (scratch, head) = edited();

  let status = scratch.read_tool("git_status", &json!({}));
  assert_eq!(status["branch"], "main", "{status}");
  assert_eq!(status["upstream"], "origin/main");
  assert_eq!(status["ahead"], 1, "one commit not pushed: {status}");
  assert_eq!(status["behind"], 0);
  assert_eq!(status["head"], head.as_str());
  assert_eq!(status["clean"], false);
  let files = status["files"].as_array().expect("files");
  assert!(
    files
      .iter()
      .any(|file| file["path"] == "README.md" && file["unstaged"] == "modified"),
    "{status}"
  );
  assert!(
    files
      .iter()
      .any(|file| file["path"] == "notes.txt" && file["untracked"] == true),
    "{status}"
  );
  assert_eq!(status["in_progress"], serde_json::Value::Null);
}

#[test]
fn orchestrator_git_reads_log_diff_show_branches_blame_and_remotes() {
  let (scratch, head) = edited();
  let log = scratch.read_tool("git_log", &json!({ "limit": 1 }));
  let commits = log["commits"].as_array().expect("commits");
  assert_eq!(commits.len(), 1, "the limit holds");
  assert_eq!(commits[0]["subject"], "Add one");
  assert_eq!(commits[0]["commit"], head.as_str());
  let ranged = scratch.read_tool("git_log", &json!({ "range": "origin/main..main" }));
  assert_eq!(ranged["commits"].as_array().map(Vec::len), Some(1), "{ranged}");
  let by_path = scratch.read_tool("git_log", &json!({ "path": "README.md" }));
  assert_eq!(by_path["commits"][0]["subject"], "Initial commit");

  let diff = scratch.read_tool("git_diff", &json!({}));
  let text = diff["text"].as_str().expect("a patch");
  assert!(text.contains("+Kontur, edited"), "{text}");
  assert_eq!(diff["truncated"], false);
  let stat = scratch.read_tool(
    "git_diff",
    &json!({ "from": "origin/main", "to": "main", "stat": true }),
  );
  assert!(stat["text"].as_str().expect("counts").contains("src/lib.rs"), "{stat}");

  let shown = scratch.read_tool("git_show", &json!({ "rev": "HEAD", "stat": true }));
  let shown = shown["text"].as_str().expect("a commit");
  assert!(shown.contains("Add one") && shown.contains("src/lib.rs"), "{shown}");
  let file = scratch.read_tool("git_show", &json!({ "rev": "HEAD", "file": "src/lib.rs" }));
  assert_eq!(file["text"], "pub fn one() {}\n");

  let branches = scratch.read_tool("git_branches", &json!({}));
  let main = &branches["local"][0];
  assert_eq!(main["name"], "main", "{branches}");
  assert_eq!(main["current"], true);
  assert_eq!(main["upstream"], "origin/main");
  assert_eq!(main["ahead"], 1);
  assert_eq!(branches["remote"][0]["name"], "origin/main");

  let blame = scratch.read_tool("git_blame", &json!({ "path": "src/lib.rs", "start": 1, "end": 1 }));
  assert!(
    blame["text"].as_str().expect("lines").contains("Test Author"),
    "{blame}"
  );

  let origin = scratch.origin().to_string_lossy().into_owned();
  scratch.git(&[
    "remote",
    "add",
    "mirror",
    "https://someone:secret-token@example.com/kontur.git",
  ]);
  let remotes = scratch.read_tool("git_remotes", &json!({}));
  let listed = remotes["remotes"].as_array().expect("remotes");
  let origin_listed = listed.iter().find(|remote| remote["name"] == "origin").expect("origin");
  assert_eq!(origin_listed["fetch"], origin.as_str());
  assert_eq!(origin_listed["push"], origin.as_str());
  assert!(
    !remotes.to_string().contains("secret-token"),
    "credentials in a URL are never shown: {remotes}"
  );

  assert_eq!(scratch.head("HEAD"), head, "reading changes nothing");
  assert_eq!(scratch.read("README.md"), "Kontur, edited\n");
}

#[test]
fn orchestrator_git_reads_refuse_an_option_where_a_revision_goes() {
  let scratch = Scratch::new();
  let called = scratch.call("git_log", &json!({ "range": "--output=stolen.txt" }), false);
  assert!(
    matches!(
      called,
      Err(orchester_workspace::app_mcp::ToolError::InvalidArguments(_))
    ),
    "{called:?}"
  );
  assert!(
    !scratch.root().join("stolen.txt").exists(),
    "git never read it as an option"
  );
}

#[test]
fn orchestrator_git_reads_refuse_a_folder_that_is_no_repository() {
  let scratch = Scratch::new();
  let plain = scratch.dir.path().join("plain");
  std::fs::create_dir_all(&plain).expect("a plain folder");
  let called = orchester_workspace::orchestrator::git::call(
    &scratch.programs,
    &plain,
    ("git_status", &serde_json::Map::new()),
    (false, orchester_workspace::orchestrator::git::Timeouts::STANDARD),
  );
  assert_eq!(super::scratch::refused(called), "not_a_repository");
}

#[test]
fn orchestrator_git_reads_blame_at_a_revision() {
  let scratch = Scratch::new();
  scratch.commit("src/lib.rs", "pub fn one() {}\npub fn two() {}\n", "Add one and two");
  let added = scratch.head("HEAD");
  scratch.commit("src/lib.rs", "pub fn one() {}\npub fn three() {}\n", "Three");
  let whole = scratch.read_tool("git_blame", &json!({ "path": "src/lib.rs", "rev": added }));
  let text = whole["text"].as_str().expect("lines");
  assert!(text.contains("two()") && !text.contains("three()"), "{text}");
  let line = scratch.read_tool(
    "git_blame",
    &json!({ "path": "src/lib.rs", "rev": "HEAD~1", "start": 2, "end": 2 }),
  );
  let text = line["text"].as_str().expect("a line");
  assert_eq!(text.lines().count(), 1, "{text}");
  assert!(text.contains("two()"), "{text}");
}

#[test]
fn orchestrator_git_reads_a_cut_log_without_a_partial_commit() {
  let scratch = Scratch::new();
  let subject = "s".repeat(600);
  let mut stream = String::new();
  for mark in 1..=150 {
    let parent = if mark == 1 {
      "from refs/heads/main^0\n".to_owned()
    } else {
      format!("from :{}\n", mark - 1)
    };
    stream.push_str(&format!(
      "commit refs/heads/main\nmark :{mark}\ncommitter Test Author <author@example.com> 1700000000 +0000\ndata \
       {len}\n{subject}\n{parent}\n",
      len = subject.len(),
    ));
  }
  scratch.git_with_input(&["fast-import", "--quiet"], &stream);
  scratch.git(&["reset", "--hard", "main"]);
  let log = scratch.read_tool("git_log", &json!({ "limit": 200 }));
  assert_eq!(log["truncated"], true, "150 commits of 600 bytes pass the cap");
  let commits = log["commits"].as_array().expect("commits");
  assert!(!commits.is_empty() && commits.len() < 150, "{}", commits.len());
  for commit in commits {
    assert_eq!(commit["subject"], subject.as_str(), "no commit is told half");
    assert_eq!(commit["commit"].as_str().map(str::len), Some(40));
  }
}
