use super::{
  decision::{connection, open},
  harness::Seeded,
};

fn edit(seeded: &mut Seeded) {
  seeded.click("decision-more");
  seeded.click("decision-edit");
  seeded.until("editor", |s| s.exists("decision-editor"));
}

#[test]
fn connections_decision_edit_cancel_keeps_storage_and_save_renames_with_models_kept() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, orchester_core::connection::DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  edit(&mut seeded);
  seeded.set("decision-edit-name", "Renamed");
  seeded.click("decision-edit-cancel");
  seeded.until("page", |s| s.exists("decision-connection-page"));
  assert!(seeded.text("decision-title").contains("Decision server"));
  edit(&mut seeded);
  seeded.set("decision-edit-name", "Renamed");
  seeded.click("decision-edit-save");
  seeded.until("saved page", |s| {
    s.exists("decision-connection-page") && s.text("decision-title").contains("Renamed")
  });
  let saved = seeded
    .db()
    .decision_connection(connection.id())
    .expect("read")
    .expect("stored");
  assert_eq!(saved.models(), connection.models());
  assert_eq!(saved.last_check(), connection.last_check());
}

#[test]
fn connections_decision_edit_check_does_not_save_and_local_synthetic_save_clears_old_models() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, orchester_core::connection::DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  edit(&mut seeded);
  seeded.set("decision-edit-url", "http://127.0.0.1:2/v1");
  seeded.click("decision-edit-check");
  seeded.until("draft check answer", |s| {
    s.text("decision-edit-models-row").contains("Nothing answers")
  });
  assert_eq!(
    seeded
      .db()
      .decision_connection(connection.id())
      .expect("read")
      .expect("stored")
      .base_url(),
    connection.base_url()
  );
  lurq::app::events::with_synthetic_input(|| seeded.click("decision-edit-save"));
  seeded.until("saved page", |s| s.exists("decision-connection-page"));
  let stored = seeded
    .db()
    .decision_connection(connection.id())
    .expect("read")
    .expect("stored");
  assert_eq!(stored.base_url().as_str(), "http://127.0.0.1:2/v1");
  assert!(stored.models().is_empty());
  assert!(stored.last_check().is_none());
}

#[test]
fn connections_decision_edit_hosted_rejects_synthetic_save_even_when_changed_to_local() {
  let mut seeded = Seeded::new("connections");
  let mut connection = connection(&seeded, orchester_core::connection::DecisionCheckResult::KeyRefused);
  connection.set_server(
    orchester_core::connection::DecisionServerUrl::new("https://api.typesafe.ai/v1").expect("url"),
    None,
  );
  seeded.db().save_decision_connection(&connection).expect("hosted");
  open(&mut seeded, connection.id());
  edit(&mut seeded);
  assert!(seeded.text("decision-edit-destination").contains("api.typesafe.ai"));
  seeded.set("decision-edit-url", "http://127.0.0.1:1/v1");
  lurq::app::events::with_synthetic_input(|| seeded.click("decision-edit-save"));
  assert!(seeded.exists("decision-editor"));
  assert!(seeded.text("decision-edit-actions").contains("only you"));
  assert_eq!(
    seeded
      .db()
      .decision_connection(connection.id())
      .expect("read")
      .expect("stored")
      .base_url(),
    connection.base_url()
  );
  seeded.click("decision-edit-save");
  seeded.until("person saved", |s| s.exists("decision-connection-page"));
}

#[test]
fn connections_decision_repair_headers_open_editor_and_invalid_url_cannot_save() {
  let mut seeded = Seeded::new("connections");
  for result in [
    orchester_core::connection::DecisionCheckResult::KeyRefused,
    orchester_core::connection::DecisionCheckResult::NotSystemOneApi,
  ] {
    let connection = connection(&seeded, result);
    open(&mut seeded, connection.id());
    seeded.click("decision-repair");
    seeded.until("editor", |s| s.exists("decision-editor"));
    seeded.set("decision-edit-url", "https://user:password@example.test/v1");
    seeded.click("decision-edit-save");
    assert!(seeded.exists("decision-editor"));
    assert_eq!(
      seeded
        .db()
        .decision_connection(connection.id())
        .expect("read")
        .expect("stored")
        .base_url(),
      connection.base_url()
    );
    seeded.click("decision-edit-cancel");
    seeded
      .db()
      .delete_decision_connection(connection.id())
      .expect("remove fixture");
  }
}

#[test]
fn connections_decision_edit_reuses_secret_picker_and_redacts_new_key_on_cancel() {
  use orchester_core::store::SecretStore;
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, orchester_core::connection::DecisionCheckResult::Connected);
  let before = seeded.db().secrets().expect("secrets").len();
  open(&mut seeded, connection.id());
  edit(&mut seeded);
  super::api_provider::choose(&mut seeded, "connection-provider-key", "SENTRY_TOKEN (Development)");
  seeded.click("decision-edit-save");
  seeded.until("saved key", |s| {
    s.exists("decision-connection-page") && s.text("decision-key-name").contains("SENTRY_TOKEN")
  });
  edit(&mut seeded);
  seeded.click("connection-provider-new-secret");
  seeded.set("connection-provider-secret-name", "NEW_DECISION_KEY");
  seeded.set("connection-provider-secret-value", "synthetic-form-only-value");
  assert!(
    seeded
      .window
      .tree
      .get_element_by_id_mut("connection-provider-secret-value")
      .and_then(|element| element.as_text_input())
      .expect("credential input")
      .is_masked()
  );
  // ElementRef can read the native buffer; the reused masked control redacts MCP inspection.
  assert!(
    !seeded
      .text("decision-edit-destination")
      .contains("synthetic-form-only-value")
  );
  seeded.click("decision-edit-cancel");
  seeded.until("cancelled", |s| s.exists("decision-connection-page"));
  assert_eq!(seeded.db().secrets().expect("secrets").len(), before);
}

#[test]
fn connections_decision_edit_draft_check_refuses_an_unsaved_hosted_destination() {
  let mut seeded = Seeded::new("connections");
  let connection = connection(&seeded, orchester_core::connection::DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  edit(&mut seeded);
  seeded.set("decision-edit-url", "https://example.test/v1");
  lurq::app::events::with_synthetic_input(|| seeded.click("decision-edit-check"));
  seeded.until("person-only check refusal", |s| {
    s.text("decision-edit-actions").contains("Only a person")
  });
  assert_eq!(
    seeded
      .db()
      .decision_connection(connection.id())
      .expect("read")
      .expect("stored")
      .base_url(),
    connection.base_url()
  );
}

#[test]
fn connections_decision_edit_rejects_concurrent_server_changes_from_originally_displayed_version() {
  let mut seeded = Seeded::new("connections");
  let mut connection = connection(&seeded, orchester_core::connection::DecisionCheckResult::Connected);
  open(&mut seeded, connection.id());
  edit(&mut seeded);
  seeded.set("decision-edit-name", "My rename");
  connection.set_server(
    orchester_core::connection::DecisionServerUrl::new("http://127.0.0.1:2/v1").expect("url"),
    None,
  );
  connection.record_models(vec![
    orchester_core::resource::ModelId::new("new-model").expect("model"),
  ]);
  seeded
    .db()
    .save_decision_connection(&connection)
    .expect("concurrent edit");
  seeded
    .window
    .projects()
    .feed(seeded.project)
    .outside
    .update(|counts| counts.connections += 1);
  seeded.until("updated props", |s| {
    !s.text("decision-edit-models-row").contains("jev-latest")
  });
  seeded.click("decision-edit-save");
  seeded.until("conflict shown", |s| {
    s.text("decision-edit-actions")
      .contains("changed while you were editing")
  });
  assert!(seeded.exists("decision-editor"));
  assert_eq!(
    seeded.db().decision_connection(connection.id()).expect("read"),
    Some(connection)
  );
  seeded.click("decision-edit-cancel");
  seeded.until("current page", |s| {
    s.exists("decision-url-value") && s.text("decision-url-value").contains("127.0.0.1:2")
  });
}
