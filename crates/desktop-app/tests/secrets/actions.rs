//! Writes through the domain, masked retry, pending locks, archived references and person-only replacement.
use std::sync::atomic::Ordering;

use orchester_core::{
  secret::{Environment, VaultKey},
  store::SecretStore,
};

use super::{
  harness::{NEXT, Secrets, VALUE},
  support::project_id,
};

#[test]
fn secrets_add_rotate_and_archive_remove_the_value_but_preserve_metadata_and_references() {
  let mut s = Secrets::new();
  s.list();
  s.click("secrets-add");
  s.set("secret-name", "DOCS_TOKEN");
  s.set("secret-value", VALUE);
  s.private();
  s.click("secret-save");
  s.until("saved", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  let secret = s.db().secrets().expect("metadata").remove(0);
  let key = VaultKey::new(project_id(), secret.id());
  assert!(s.vault.contains(&key, VALUE));
  s.click("secret-replace");
  s.set("secret-value", NEXT);
  s.private();
  s.click("secret-save");
  s.until("replaced", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  assert!(s.vault.contains(&key, NEXT));
  assert!(
    s.db()
      .secret(secret.id())
      .expect("metadata")
      .expect("secret")
      .rotated_at()
      .is_some()
  );
  s.bind(&secret);
  s.refresh();
  s.until("binding", |s| s.text("secret-uses").contains("kontur-docs"));
  s.click("secret-archive");
  assert!(s.text("secret-inspector").contains("Archive DOCS_TOKEN?"));
  assert!(s.text("secret-inspector").contains("1 MCP server uses"));
  s.click("secret-archive-cancel");
  assert!(!s.text("secret-inspector").contains("Archive DOCS_TOKEN?"));
  s.click("secret-archive");
  s.click("secret-archive");
  s.until("archived", |s| s.exists("secret-archived"));
  assert!(s.db().secrets().expect("active secrets").is_empty());
  assert!(
    s.db()
      .secret(secret.id())
      .expect("metadata")
      .expect("secret")
      .is_archived()
  );
  assert!(!s.vault.contains(&key, NEXT));
  assert!(s.text("secret-uses").contains("MCP · kontur-docs"));
  assert!(!s.exists("secret-replace"));
  assert!(!s.exists("secret-archive"));
  assert!(!s.text("app-window").contains(NEXT));
  s.click("secrets-add");
  s.set("secret-name", "DOCS_TOKEN");
  s.set("secret-value", VALUE);
  assert!(!s.exists("secret-save-reason"), "the archived name is reusable");
}

#[test]
fn secrets_failed_save_and_replace_keep_the_masked_input_for_retry() {
  let mut s = Secrets::new();
  s.list();
  s.click("secrets-add");
  s.set("secret-name", "DOCS_TOKEN");
  s.set("secret-value", VALUE);
  s.vault.fail.store(true, Ordering::SeqCst);
  s.click("secret-save");
  s.until("failed save", |s| s.exists("secret-save-error"));
  s.private();
  assert!(s.db().secrets().expect("metadata").is_empty());
  s.vault.fail.store(false, Ordering::SeqCst);
  s.click("secret-save");
  s.until("retry saved", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  let secret = s.db().secrets().expect("metadata").remove(0);
  let key = VaultKey::new(project_id(), secret.id());
  assert!(s.vault.contains(&key, VALUE), "retry uses the retained entry");
  s.click("secret-replace");
  s.set("secret-value", NEXT);
  s.vault.fail.store(true, Ordering::SeqCst);
  s.click("secret-save");
  s.until("failed replace", |s| s.exists("secret-save-error"));
  s.private();
  assert!(
    s.vault.contains(&key, VALUE),
    "failed rotation leaves the previous value"
  );
  s.vault.fail.store(false, Ordering::SeqCst);
  s.click("secret-save");
  s.until("retry replaced", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  assert!(s.vault.contains(&key, NEXT));
}

#[test]
fn secrets_pending_save_is_single_and_locks_fields_and_cancel() {
  let mut s = Secrets::new();
  s.list();
  s.click("secrets-add");
  s.set("secret-name", "DOCS_TOKEN");
  s.set("secret-value", VALUE);
  s.vault.block();
  s.click("secret-save");
  s.until("pending save", |s| s.text("secret-save").contains("Saving"));
  s.private();
  s.click("secret-save");
  s.click("secret-cancel");
  assert!(s.exists("secret-value"), "pending cancel is disabled");
  s.vault.release();
  s.until("saved", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  assert_eq!(
    s.vault.stores.load(Ordering::SeqCst),
    1,
    "one write despite repeat presses"
  );
  assert_eq!(s.db().secrets().expect("metadata")[0].name().as_str(), "DOCS_TOKEN");
}

#[test]
fn secrets_used_replacement_requires_a_person_and_releases_the_input_gate() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.bind(&secret);
  s.list();
  s.click("secret-replace");
  s.set("secret-value", NEXT);
  s.click("secret-save");
  s.until("replacement question", |s| {
    s.exists("secret-confirm-replacement-confirm")
  });
  let gate = s.window.link.input_gate();
  assert!(gate.protecting(), "the window input lock covers the in-place question");
  assert_eq!(
    s.vault.stores.load(Ordering::SeqCst),
    1,
    "not replaced before confirmation"
  );
  s.private();
  s.click("secret-confirm-replacement-keep");
  assert!(!gate.protecting());
  s.click("secret-save");
  s.until("armed question", |s| {
    s.window.tree.tick_futures();
    s.exists("secret-confirm-replacement-confirm")
  });
  s.window.pass();
  s.click("secret-confirm-replacement-confirm");
  s.until("replaced", |s| {
    s.exists("secret-inspector-name") && !s.exists("secret-value")
  });
  assert!(!gate.protecting());
  assert!(s.vault.contains(&VaultKey::new(project_id(), secret.id()), NEXT));
}

#[test]
fn secrets_archive_failure_keeps_active_metadata_and_allows_retry() {
  let mut s = Secrets::new();
  let secret = s.seed("DOCS_TOKEN", Environment::Development);
  s.list();
  s.click("secret-archive");
  s.vault.fail.store(true, Ordering::SeqCst);
  s.click("secret-archive");
  s.until("archive denied", |s| s.exists("secret-archive-error"));
  assert!(
    !s.db()
      .secret(secret.id())
      .expect("metadata")
      .expect("secret")
      .is_archived()
  );
  assert!(s.vault.contains(&VaultKey::new(project_id(), secret.id()), VALUE));
  s.vault.fail.store(false, Ordering::SeqCst);
  s.click("secret-archive");
  s.until("archive retried", |s| s.exists("secret-archived"));
}
