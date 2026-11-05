use orchester_app::routes::{NewConnection, ProjectPage, Route};

use super::harness::Seeded;

fn add(seeded: &mut Seeded) {
  seeded.list();
  seeded.click("connections-add-decision");
  seeded.until("Add decision model", |s| s.exists("decision-server-choices"));
}

#[test]
fn connections_decision_add_hosted_requires_person_and_cancel_discards_draft() {
  let mut seeded = Seeded::new("connections");
  add(&mut seeded);
  assert_eq!(
    seeded.window.navigation().route_now(),
    Route::Project(seeded.project, ProjectPage::NewConnection(NewConnection::DecisionModel))
  );
  seeded.set("decision-edit-name", "Hosted decisions");
  let notice = seeded.text("decision-edit-destination");
  for words in [
    "api.typesafe.ai",
    "command or tool call",
    "last messages",
    "Secret values never",
  ] {
    assert!(notice.contains(words), "{words}");
  }
  lurq::app::events::with_synthetic_input(|| seeded.click("decision-edit-save"));
  assert!(seeded.text("decision-edit-actions").contains("only you"));
  assert!(seeded.db().decision_connections().expect("read").is_empty());
  seeded.click("decision-edit-cancel");
  seeded.until("connections", |s| s.exists("connections-list"));
  assert!(seeded.db().decision_connections().expect("read").is_empty());
  add(&mut seeded);
  seeded.set("decision-edit-name", "Hosted decisions");
  seeded.click("decision-edit-save");
  seeded.until("saved", |s| !s.db().decision_connections().expect("read").is_empty());
  let saved = seeded.db().decision_connections().expect("read");
  assert_eq!(saved[0].base_url().as_str(), "https://api.typesafe.ai/v1");
  assert!(saved[0].key().is_none());
}

#[test]
fn connections_decision_add_local_checks_without_saving_and_synthetic_save_works() {
  let mut seeded = Seeded::new("connections");
  add(&mut seeded);
  seeded.click("decision-server-laya");
  assert!(seeded.text("decision-edit-destination").contains("api.laya.studio"));
  seeded.click("decision-server-local");
  assert!(seeded.text("decision-edit-destination").contains("nothing leaves"));
  assert!(!seeded.text("decision-edit-actions").contains("only you"));
  seeded.set("decision-edit-name", "Local decisions");
  seeded.set("decision-edit-url", "http://127.0.0.1:1/v1");
  seeded.click("decision-edit-check");
  seeded.until("check", |s| {
    s.text("decision-edit-models-row").contains("Nothing answers")
  });
  assert!(seeded.db().decision_connections().expect("read").is_empty());
  lurq::app::events::with_synthetic_input(|| seeded.click("decision-edit-save"));
  seeded.until("saved", |s| !s.db().decision_connections().expect("read").is_empty());
  let saved = seeded.db().decision_connections().expect("read");
  assert_eq!(saved[0].name().as_str(), "Local decisions");
  assert!(saved[0].is_on_this_computer());
  assert!(saved[0].key().is_none());
}

#[test]
fn connections_decision_add_lists_real_catalog_and_clears_it_after_server_changes() {
  use std::{
    io::{Read, Write},
    net::TcpListener,
  };
  let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
  let url = format!("http://{}/v1", listener.local_addr().expect("address"));
  let server = std::thread::spawn(move || {
    let (mut socket, _) = listener.accept().expect("accept");
    socket
      .set_read_timeout(Some(std::time::Duration::from_secs(10)))
      .expect("timeout");
    let mut request = [0; 4096];
    let length = socket.read(&mut request).expect("request");
    assert!(String::from_utf8_lossy(&request[..length]).starts_with("GET /v1/models "));
    let body = r#"{"models":[{"name":"jev-latest","description":"","release_date":""}]}"#;
    write!(
      socket,
      "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
      body.len()
    )
    .expect("response");
  });
  let mut seeded = Seeded::new("connections");
  add(&mut seeded);
  seeded.click("decision-server-other");
  seeded.set("decision-edit-name", "Catalog test");
  seeded.set("decision-edit-url", &url);
  seeded.click("decision-edit-check");
  seeded.until("models", |s| s.text("decision-edit-models-row").contains("jev-latest"));
  server.join().expect("server finished");
  assert!(seeded.db().decision_connections().expect("read").is_empty());
  seeded.click("decision-server-local");
  assert!(!seeded.text("decision-edit-models-row").contains("jev-latest"));
}
