//! An edit Git's stat cache hides (same size and modification time, in a repository that does not trust change
//! times) after the person reviewed a commit: the commit is refused as stale, and the page names the path the hashed
//! check found changed, without writing the person's index.

use std::{
  fs,
  time::{Duration, SystemTime},
};

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

fn set_modified(path: &std::path::Path, modified: SystemTime) {
  fs::File::options()
    .write(true)
    .open(path)
    .expect("file")
    .set_modified(modified)
    .expect("mtime");
}

#[test]
fn local_integration_stale_refusal_names_an_edit_the_stat_cache_hides() {
  let fixture = Fixture::new(false);
  let root = fixture.root().to_owned();
  git(&root, &["config", "core.trustctime", "false"]);
  fs::write(root.join("local.txt"), "selected staged\n").expect("edit");
  git(&root, &["add", "local.txt"]);
  let hidden = root.join("task.txt");
  let old = SystemTime::now() - Duration::from_secs(60);
  set_modified(&hidden, old);
  git(&root, &["update-index", "--refresh"]);
  let mut page = Page::new(fixture);
  page.window.tree.resize(1440, 1080);
  page.ready("local-select-local_integration.staged-local.txt");
  page.click("local-select-local_integration.staged-local.txt");
  page.click("local-review-commit");
  page.ready("local-commit-message");
  fs::write(&hidden, "TASK BASE\n").expect("same-size edit");
  set_modified(&hidden, old);
  assert!(
    !git(&root, &["status", "--porcelain"]).contains("task.txt"),
    "Git's own stat cache hides the edit"
  );
  let index = fs::read(root.join(".git/index")).expect("index");
  page.type_into("local-commit-message", "reviewed commit");
  page.click("local-confirm-scope");
  page.click("local-confirm");
  page.until("the refusal names the hidden edit", |window| {
    window
      .texts("local-work-error")
      .is_some_and(|texts| texts.iter().any(|text| text.contains("task.txt")))
  });
  let said = page.window.texts("local-work-error").expect("refusal").join(" ");
  assert!(said.contains("main changed after your review"), "{said}");
  assert_eq!(
    fs::read(root.join(".git/index")).expect("index"),
    index,
    "the person's index is not written"
  );
  assert_eq!(git(&root, &["show", ":local.txt"]), "selected staged");
  assert_eq!(
    git(&root, &["log", "-1", "--format=%s"]),
    "base",
    "nothing was committed"
  );
}
