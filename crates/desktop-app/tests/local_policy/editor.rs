use std::fs;

use lurq::app::events::with_synthetic_input;
use orchester_core::integration::local::{DraftChoice, LocalPhase};

use super::{
  client::{Client, ok},
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn conflict() -> Page {
  let fixture = Fixture::new(false);
  fs::write(fixture.task_root.join("z.txt"), "task second conflict\n").expect("task");
  git(&fixture.task_root, &["add", "z.txt"]);
  git(&fixture.task_root, &["commit", "-m", "second conflict"]);
  fs::write(fixture.root().join("z.txt"), "main second conflict\n").expect("main");
  git(fixture.root(), &["add", "z.txt"]);
  fs::write(fixture.root().join("task.txt"), "main conflicting bytes\n").expect("main");
  git(fixture.root(), &["commit", "-am", "main work"]);
  let mut session = fixture.open();
  session.prepare_clean().expect("prepare");
  session.sync().expect("owned conflict");
  assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
  drop(session);
  let mut page = Page::new(fixture);
  page.window.tree.resize(1440, 1300);
  page.window.pass();
  page.ready("local-result-editor");
  page
}

#[test]
fn local_policy_ordinary_mcp_candidate_save_with_optins_off_and_on_never_applies_or_stages() {
  for allowed in [false, true] {
    let mut page = conflict();
    let workspace = page.window.launch.workspace().expect("workspace").clone();
    workspace
      .set_automation_changes_permissions(allowed)
      .expect("private setting");
    workspace
      .set_automation_answers_approvals(allowed)
      .expect("private setting");
    let client = Client::new(&mut page, true);
    assert!(
      !page.window.link.input_gate().protecting(),
      "unresolved editor is ordinary"
    );
    let main = git(page.root(), &["rev-parse", "HEAD"]);
    let task = git(&page.fixture.task_root, &["rev-parse", "HEAD"]);
    let index = git(&page.fixture.task_root, &["ls-files", "--stage"]);
    let bytes = fs::read(page.fixture.task_root.join("task.txt")).expect("actual markers");
    ok(&client.click(&mut page, "local-use-current"));
    page.until("candidate choice", |window| {
      window.tree.get_element_by_id("local-candidate-choice").is_some()
    });
    assert_eq!(fs::read(page.fixture.task_root.join("task.txt")).expect("file"), bytes);
    assert_eq!(git(&page.fixture.task_root, &["ls-files", "--stage"]), index);
    let draft = "<<<<<<< unfinished raw editor\nkeep these markers\n=======\nnot resolved\n>>>>>>> raw\n";
    ok(&client.set(&mut page, "local-result-editor", draft));
    ok(&client.set(&mut page, "local-destination", "../partial/"));
    assert!(page.has("local-unsaved"));
    page.choose("local-conflict-picker", "z.txt");
    assert_eq!(
      page.value("local-result-editor"),
      draft,
      "a dirty candidate cannot switch files"
    );
    page.window.navigation().go(orchester_app::routes::Route::AllProjects);
    page.window.pass();
    assert_eq!(page.window.navigation().route_now(), page.route);
    ok(&client.click(&mut page, "local-keep-editing"));
    assert_eq!(page.value("local-destination"), "../partial/");
    page.window.navigation().go(orchester_app::routes::Route::AllProjects);
    page.window.pass();
    ok(&client.click(&mut page, "local-save-pause"));
    page.until("durable save before leave", |window| {
      window.navigation().route_now() == orchester_app::routes::Route::AllProjects
    });
    let mut session = page.fixture.open();
    session.reconcile().expect("resume owned conflict");
    let file = orchester_exec::local_git::LocalGit::new(
      &page.fixture.task_root,
      &page.fixture.task_root.join(".orchester/local-integration"),
    )
    .expect("private Git")
    .conflicts()
    .expect("still unresolved")
    .remove(0);
    let expected = orchester_exec::local_git::file_hash(&page.fixture.task_root, &file.path).expect("hash");
    let saved = session
      .editor_draft(&file, expected.as_ref())
      .expect("draft")
      .expect("saved");
    assert_eq!(saved.text(), draft);
    assert_eq!(saved.destination(), "../partial/");
    assert_eq!(
      saved.choice(),
      DraftChoice::Text,
      "later text editing supersedes a version choice"
    );
    assert_eq!(session.operation().phase(), LocalPhase::TaskConflicts);
    assert!(session.operation().checked().is_none());
    assert!(session.operation().merged().is_none());
    assert_eq!(git(page.root(), &["rev-parse", "HEAD"]), main);
    assert_eq!(git(&page.fixture.task_root, &["rev-parse", "HEAD"]), task);
    assert_eq!(git(&page.fixture.task_root, &["ls-files", "--stage"]), index);
    assert_eq!(fs::read(page.fixture.task_root.join("task.txt")).expect("file"), bytes);
  }
}

#[test]
fn local_policy_queued_reviewed_save_does_not_become_native_after_synthetic_callback() {
  let mut page = conflict();
  page.window.link.input_gate().bind_window_control(true);
  let before = fs::read(page.fixture.task_root.join("task.txt")).expect("bytes");
  let index = git(&page.fixture.task_root, &["ls-files", "--stage"]);
  page.set("local-destination", "../native partial/");
  page.type_into("local-result-editor", "reviewed candidate only");
  with_synthetic_input(|| page.click("local-save-result"));
  page.until("explicit reviewed refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  assert_eq!(
    fs::read(page.fixture.task_root.join("task.txt")).expect("unchanged"),
    before
  );
  assert_eq!(git(&page.fixture.task_root, &["ls-files", "--stage"]), index);
  assert_eq!(page.value("local-destination"), "../native partial/");
  assert!(
    page
      .window
      .texts("local-work-error")
      .expect("visible refusal")
      .iter()
      .any(|word| word.contains("person"))
  );
}

/// A review whose one untracked file the person has selected natively.
fn selected() -> Page {
  let fixture = Fixture::new(false);
  fs::write(fixture.root().join("note.txt"), "personal note\n").expect("untracked");
  let mut page = Page::new(fixture);
  page.ready("local-select-local_integration.untracked-note.txt");
  page.click("local-select-local_integration.untracked-note.txt");
  page
}

fn no_operation(page: &Page) -> bool {
  orchester_core::integration::local::LocalIntegrationStore::local_operation(
    &orchester_store::ProjectDb::open(page.root()).expect("db"),
    page.run(),
  )
  .expect("journal")
  .is_none()
}

#[test]
fn local_policy_commit_review_scope_off_and_other_tab_questions_have_real_outcomes() {
  let mut page = selected();
  let client = Client::new(&mut page, true);
  let head = git(page.root(), &["rev-parse", "HEAD"]);
  let secret = page
    .window
    .link
    .input_gate()
    .protect(orchester_app::mcp::input_gate::Guarded::WideningQuestion);
  let refused = client.click(&mut page, "local-review-commit");
  assert_eq!(super::client::value(&refused)["code"], "person_only");
  assert!(no_operation(&page), "a refused candidate records nothing");
  drop(secret);
  ok(&client.click(&mut page, "local-review-commit"));
  page.ready("local-commit-message-label");
  assert!(!no_operation(&page), "the candidate records the current review");
  assert_eq!(
    git(page.root(), &["rev-parse", "HEAD"]),
    head,
    "a candidate commits nothing"
  );
  let mut separate = selected();
  let off = Client::new(&mut separate, false);
  let refused = off.click(&mut separate, "local-review-commit");
  assert!(refused["isError"].as_bool().unwrap_or(false), "no window control scope");
  assert!(!separate.has("local-commit-message-label"));
  assert!(no_operation(&separate), "no operation created by refused input");
}
#[test]
fn local_policy_unscoped_synthetic_candidate_and_keep_are_visibly_refused_without_mutation() {
  let mut page = conflict();
  let before = fs::read(page.fixture.task_root.join("task.txt")).expect("original");
  let index = git(&page.fixture.task_root, &["ls-files", "--stage"]);
  let initial = page.value("local-result-editor");
  with_synthetic_input(|| page.click("local-use-current"));
  page.until("candidate scope refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  assert_eq!(page.value("local-result-editor"), initial);
  assert!(!page.has("local-candidate-choice"));
  page.type_into("local-result-editor", "native private unfinished candidate");
  page.window.navigation().go(orchester_app::routes::Route::AllProjects);
  page.window.pass();
  with_synthetic_input(|| page.click("local-keep-editing"));
  page.until("keep scope refusal", |window| {
    window.texts("local-work-error").is_some()
  });
  assert_eq!(page.window.navigation().route_now(), page.route);
  assert_eq!(page.value("local-result-editor"), "native private unfinished candidate");
  assert_eq!(
    fs::read(page.fixture.task_root.join("task.txt")).expect("retained"),
    before
  );
  assert_eq!(git(&page.fixture.task_root, &["ls-files", "--stage"]), index);
  assert!(
    page
      .window
      .texts("local-work-error")
      .expect("visible refusal")
      .iter()
      .any(|word| word.contains("authorization"))
  );
}
