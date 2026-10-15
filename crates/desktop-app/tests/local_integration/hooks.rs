//! A post-commit hook that moves HEAD after the person's reviewed commit landed: the page says so instead of taking
//! the moved HEAD as reviewed.

use std::fs;

use super::{
  domain_support::{Fixture, git},
  integration_support::Page,
};

#[test]
fn local_integration_a_hook_that_moves_head_after_the_commit_is_shown() {
  let fixture = Fixture::new(false);
  let root = fixture.root().to_owned();
  fs::write(root.join("local.txt"), "selected staged\n").expect("edit");
  git(&root, &["add", "local.txt"]);
  // `--no-verify` does not skip post-commit: the hook's own amend would run it again, so it amends once.
  fs::write(
    root.join(".git/hooks/post-commit"),
    "#!/bin/sh\n[ -n \"$AMENDED\" ] || AMENDED=1 git commit --amend --no-verify --allow-empty -m 'amended by hook' >/dev/null\n",
  )
  .expect("hook");
  let mut page = Page::new(fixture);
  page.window.tree.resize(1440, 1080);
  page.ready("local-select-local_integration.staged-local.txt");
  page.click("local-select-local_integration.staged-local.txt");
  page.click("local-review-commit");
  page.ready("local-commit-message");
  page.type_into("local-commit-message", "reviewed commit");
  page.click("local-confirm-scope");
  page.click("local-confirm");
  page.ready("local-hook-change");
  let notice = page.window.texts("local-hook-change").expect("notice").join(" ");
  assert!(notice.contains("a hook then moved HEAD"), "{notice}");
  assert_eq!(git(&root, &["log", "-1", "--format=%s"]), "amended by hook");
}
