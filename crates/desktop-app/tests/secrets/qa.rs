//! The native verifier's example fixtures and fault controls run real workspace transitions.
#[path = "../../examples/secrets_qa/fixtures.rs"]
mod fixtures;
#[path = "../../examples/secrets_qa/tools.rs"]
mod tools;
#[path = "../../examples/secrets_qa/vault.rs"]
mod vault;

use std::sync::Arc;

use orchester_core::secret::{SecretValue, SecretVault, VaultError};
use orchester_workspace::secrets::data::{ProjectSecrets, UseCounts};

use super::harness::Secrets;

#[test]
fn secrets_native_qa_list_and_read_failure_can_be_repaired_for_retry() {
  let s = Secrets::new();
  let repo = s.repo.parent().expect("scratch folder").join("native-fixture");
  let vault = vault::QaVault::default();
  fixtures::seed(s.window.launch.workspace().expect("workspace"), &repo, &vault, "list").expect("seed");
  let listed = ProjectSecrets::load_in(&repo).expect("metadata");
  assert_eq!(listed.in_environment(None).count(), 5);
  let counts = |name: &str| {
    let secret = listed
      .secrets
      .iter()
      .find(|secret| secret.name().as_str() == name)
      .expect("fixture secret");
    listed.use_counts(secret.id())
  };
  assert_eq!(
    counts("DOCS_API_KEY"),
    UseCounts {
      tools: 1,
      mcp_servers: 1
    }
  );
  assert_eq!(
    counts("SENTRY_TOKEN"),
    UseCounts {
      tools: 0,
      mcp_servers: 1
    }
  );
  assert_eq!(
    counts("STAGING_DB_URL"),
    UseCounts {
      tools: 1,
      mcp_servers: 0
    }
  );
  assert_eq!(counts("RELEASE_SIGNING_PASSWORD"), UseCounts::default());
  fixtures::read_failure(&repo, true).expect("damage fixture");
  assert!(ProjectSecrets::load_in(&repo).is_err());
  fixtures::read_failure(&repo, false).expect("repair fixture");
  assert_eq!(ProjectSecrets::load_in(&repo).expect("retry"), listed);
}

#[test]
fn secrets_native_qa_fill_stays_masked_and_refuses_person_only_input() {
  use orchester_app::mcp::input_gate::Guarded;
  let mut s = Secrets::new();
  s.list();
  s.click("secrets-add");
  let gate = s.window.link.input_gate();
  let vault = Arc::new(vault::QaVault::default());
  // Compiles the exact tool the native example registers; no test-only production gate.
  let _tool = tools::tool(
    vault,
    s.window
      .launch
      .workspace()
      .expect("workspace")
      .open(&s.repo)
      .expect("project"),
    s.window.launch.services().expect("services").clone(),
    gate.clone(),
  );
  tools::fill(&mut s.window.tree, &gate).expect("fill masked fixture");
  s.window.pass();
  let input = s
    .window
    .tree
    .get_element_by_id_mut("secret-value")
    .and_then(|element| element.as_text_input())
    .expect("masked input");
  assert!(input.is_masked());
  let protected = gate.protect(Guarded::WideningQuestion);
  assert_eq!(tools::fill(&mut s.window.tree, &gate), Err("person_only".into()));
  drop(protected);
  s.click("secret-cancel");
  assert!(!s.exists("secret-value"));
}

#[test]
fn secrets_native_qa_vault_failure_preserves_credential_and_archive_retry() {
  let s = Secrets::new();
  let repo = s.repo.parent().expect("scratch folder").join("native-writes");
  let vault = vault::QaVault::default();
  let open = fixtures::seed(s.window.launch.workspace().expect("workspace"), &repo, &vault, "writes").expect("seed");
  let listed = ProjectSecrets::load_in(&repo).expect("metadata");
  assert!(listed.uses.is_empty());
  let secret = &listed.secrets[0];
  let key = secret.vault_key(open.project.id());
  vault.configure(false, 1);
  assert!(matches!(vault.delete(&key), Err(VaultError::AccessDenied(_))));
  assert!(vault.entered());
  assert!(
    vault.read(&key).is_ok(),
    "failed delete preserves the stored credential"
  );
  vault.configure(false, 2);
  assert!(matches!(
    vault.store(&key, &SecretValue::new("qa-change".into()).expect("fixture")),
    Err(VaultError::Failed(_))
  ));
  vault.configure(false, 0);
  vault.delete(&key).expect("retry deletion");
  assert!(matches!(vault.read(&key), Err(VaultError::NotFound)));
}
