use std::{
  io::{self, Write},
  sync::Arc,
};

use orchester_api::ProjectCreated;
use parking_lot::Mutex;
use serde_json::json;

use crate::{fixture::*, support::TestServer};

#[derive(Clone)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl Write for Capture {
  fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    self.0.lock().extend_from_slice(bytes);
    Ok(bytes.len())
  }

  fn flush(&mut self) -> io::Result<()> {
    Ok(())
  }
}

#[tokio::test]
async fn s1b_signin_and_shared_work_logs_never_include_credentials() {
  let capture = Capture(Arc::new(Mutex::new(Vec::new())));
  let writer = capture.clone();
  let subscriber = tracing_subscriber::fmt()
    .with_ansi(false)
    .with_writer(move || writer.clone())
    .finish();
  tracing::subscriber::set_global_default(subscriber).expect("install sharing log capture");
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let response = server
    .request(
      "POST",
      "/projects",
      Some(&owner.token),
      Some(json!({
        "idempotency_key":key(), "key":"ORC", "name":"Shared project",
        "remote":"https://user:remote-credential-canary@host.example/owner/repo.git", "tasks":[upload(1)]
      })),
    )
    .await;
  assert_eq!(response.status, 201);
  assert!(!response.contains("remote-credential-canary"));
  let created = response.json::<ProjectCreated>();
  let task = &created.tasks[0];
  let claimed = changed(
    &server
      .request(
        "POST",
        &format!("{}/claim", task_path(created.project.id, task)),
        Some(&owner.token),
        Some(json!({"kind":"manual"})),
      )
      .await,
  );
  changed(
    &patch(
      &server,
      &owner.token,
      created.project.id,
      &claimed,
      json!({"priority":"high"}),
    )
    .await,
  );
  let invalid = server
    .request(
      "POST",
      "/projects",
      Some(&owner.token),
      Some(json!({
        "idempotency_key":key(), "key":"ORC", "name":"Invalid", "tasks":[],
        "remote":format!("https://host.example/owner/repo?{}",owner.token.expose())
      })),
    )
    .await;
  assert_eq!(invalid.status, 422);
  assert!(!invalid.contains(owner.token.expose()));
  server.finish().await;
  let log = String::from_utf8(capture.0.lock().clone()).expect("UTF-8 logs");
  assert!(log.contains("request") && log.contains("status=201") && log.contains("status=422"));
  assert!(!log.contains(owner.token.expose()));
  assert!(!log.contains("a secure admin password"));
  assert!(!log.contains("remote-credential-canary"));
}
