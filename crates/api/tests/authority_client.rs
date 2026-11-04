//! `Client::authority` against a fake server on loopback: the S10 reads' routes, bodies and pages, refusals typed as
//! `AuthorityError`s, and the S11 subscription's events, its `Last-Event-ID` and the ends it reports.
//!
//! `cargo test -p orchester-api --features client --test authority_client authority_client_`

use std::{
  io::{BufRead, BufReader, Read, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};

use orchester_api::{
  client::{AuthorityCallError, Client, ClientError, PlainHttp, ServerAddress, SubscriptionEnd},
  v2::{
    AuthorityErrorCode, EventCursor, ModuleListRequest, PageCursor, PageRequest, QueryScope, ResourceLibraryRequest,
    SearchText, SubscriptionEvent, TaskFilters, TaskListRequest, TaskSort,
  },
};
use orchester_core::{
  id::{ModuleId, ProjectId, ResourceId, TaskId},
  resource::VersionNumber,
  secret::SecretValue,
  task::organization::WorkflowStatus,
};
use serde_json::{Value, json};

const USER: &str = "01a183cf-b73c-71db-8a62-bd509770cd4e";

struct Answer {
  status: u16,
  content_type: &'static str,
  headers: &'static str,
  pieces: Vec<String>,
}

impl Answer {
  fn json(body: &Value) -> Self {
    Self::status(200, body)
  }

  fn status(status: u16, body: &Value) -> Self {
    Self {
      status,
      content_type: "application/json",
      headers: "",
      pieces: vec![body.to_string()],
    }
  }

  fn refusal(status: u16, code: &str) -> Self {
    Self::status(status, &json!({ "code": code, "details": {} }))
  }

  fn stream(pieces: &[&str]) -> Self {
    Self {
      status: 200,
      content_type: "text/event-stream",
      headers: "",
      pieces: pieces.iter().map(|piece| (*piece).to_owned()).collect(),
    }
  }
}

/// What the server read of one request: `METHOD path`, the headers with lower-cased names, and the body.
struct Seen {
  line: String,
  headers: Vec<(String, String)>,
  body: Vec<u8>,
}

impl Seen {
  fn header(&self, name: &str) -> Option<&str> {
    self
      .headers
      .iter()
      .find(|(header, _)| header == name)
      .map(|(_, value)| value.as_str())
  }

  fn json(&self) -> Value {
    serde_json::from_slice(&self.body).expect("a JSON body")
  }
}

struct Server {
  url: String,
  seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
  fn start(answers: Vec<Answer>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let url = format!("http://{}", listener.local_addr().expect("address"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = seen.clone();
    thread::spawn(move || {
      let mut answers = answers.into_iter();
      for mut stream in listener.incoming().flatten() {
        let Some(answer) = answers.next() else { return };
        kept.lock().expect("seen").push(read(&stream));
        // The test reads what the client made of the answer; a client that hung up fails there.
        let _ = write(&mut stream, &answer);
      }
    });
    Self { url, seen }
  }

  fn client(&self) -> Client {
    Client::new(ServerAddress::parse(&self.url).expect("address"), PlainHttp::NotAllowed).expect("client")
  }

  fn seen(&self) -> Vec<Seen> {
    std::mem::take(&mut *self.seen.lock().expect("seen"))
  }
}

fn read(stream: &TcpStream) -> Seen {
  let mut reader = BufReader::new(stream);
  let mut line = String::new();
  reader.read_line(&mut line).expect("request line");
  let line = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let mut headers = Vec::new();
  loop {
    let mut header = String::new();
    reader.read_line(&mut header).expect("header");
    let header = header.trim_end();
    if header.is_empty() {
      break;
    }
    let (name, value) = header.split_once(':').expect("a header has a name");
    headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
  }
  let length = headers
    .iter()
    .find(|(name, _)| name == "content-length")
    .map_or(0, |(_, value)| value.parse().expect("a length"));
  let mut body = vec![0; length];
  reader.read_exact(&mut body).expect("body");
  Seen { line, headers, body }
}

fn write(stream: &mut TcpStream, answer: &Answer) -> std::io::Result<()> {
  let head = format!(
    "HTTP/1.1 {} Answer\r\nContent-Type: {}\r\n{}Connection: close\r\n\r\n",
    answer.status, answer.content_type, answer.headers
  );
  stream.write_all(head.as_bytes())?;
  for piece in &answer.pieces {
    stream.write_all(piece.as_bytes())?;
    stream.flush()?;
    thread::sleep(Duration::from_millis(20));
  }
  Ok(())
}

fn token() -> SecretValue {
  SecretValue::new("token".to_owned()).expect("token")
}

fn capabilities(project: ProjectId) -> Value {
  json!({"project_id": project, "revision": 1, "role": "member",
    "actions": [{"action": "read", "subject": null, "allowed": true, "reason": null}], "unresolved": []})
}

fn task(project: ProjectId, number: u32) -> Value {
  let attribution = json!({ "by": { "id": USER, "display_name": "Alice" }, "at": 1 });
  json!({
    "id": TaskId::generate(), "number": number, "key": format!("KON-{number}"), "former_key": null, "revision": 2,
    "title": "Sign in", "brief": "", "criteria": [], "pipeline": null, "limits": null, "workflow": "in_review",
    "priority": "high", "tags": [], "blocking": [], "blocked_reason": null, "module": null, "status": "queued",
    "has_run": false, "created": attribution, "updated": attribution, "claim": null, "run": null,
    "assignee": null, "work": null, "effective_claim": null, "capabilities": capabilities(project)
  })
}

fn sse(event: &Value) -> String {
  let id = event
    .get("cursor")
    .map(|cursor| format!("id: {}\n", cursor.as_str().expect("a cursor")))
    .unwrap_or_default();
  format!(
    "{id}event: {}\ndata: {event}\n\n",
    event["kind"].as_str().expect("a kind")
  )
}

fn cursor(position: &str) -> EventCursor {
  EventCursor::new(position).expect("cursor")
}

#[tokio::test]
async fn authority_client_task_list_posts_its_request_and_reads_the_page() {
  let project = ProjectId::generate();
  let page = json!({ "items": [task(project, 12), task(project, 13)], "next": "p-2", "cursor": "41" });
  let server = Server::start(vec![Answer::json(&page)]);
  let request = TaskListRequest {
    filters: TaskFilters {
      statuses: vec![WorkflowStatus::InReview],
      search: Some(SearchText::new("sign").expect("search")),
      ..TaskFilters::default()
    },
    sort: TaskSort::Priority,
    page: PageRequest::new(Some(PageCursor::new("p-1".to_owned()).expect("cursor")), Some(2)).expect("page"),
  };
  let token = token();
  let read = server
    .client()
    .authority(&token, project)
    .tasks(&request)
    .await
    .expect("a page");
  assert_eq!(read.items.len(), 2);
  assert_eq!(read.items[0].task.key, "KON-12");
  assert_eq!(read.next.as_ref().map(PageCursor::as_str), Some("p-2"));
  assert_eq!(read.cursor, cursor("41"));
  let seen = server.seen();
  assert_eq!(seen[0].line, format!("POST /api/v2/projects/{project}/tasks/query"));
  assert_eq!(seen[0].header("authorization"), Some("Bearer token"));
  assert_eq!(seen[0].header("content-type"), Some("application/json"));
  assert_eq!(
    seen[0].json(),
    serde_json::to_value(&request).expect("the request's JSON")
  );
}

#[tokio::test]
async fn authority_client_reads_route_under_the_project() {
  let project = ProjectId::generate();
  let (task_id, module, resource) = (TaskId::generate(), ModuleId::generate(), ResourceId::generate());
  let counts = json!({"backlog":1,"todo":2,"in_progress":0,"in_review":3,"done":4,"canceled":0,"cursor":"9"});
  let empty = json!({ "items": [], "next": null, "cursor": "9" });
  let server = Server::start(vec![
    Answer::json(&counts),
    Answer::json(&capabilities(project)),
    Answer::json(&empty),
    Answer::json(&empty),
    Answer::json(&empty),
    Answer::json(&empty),
    Answer::refusal(404, "not_found"),
    Answer::refusal(404, "not_found"),
  ]);
  let (client, token) = (server.client(), token());
  let authority = client.authority(&token, project);
  let read = authority.task_counts(&TaskFilters::default()).await.expect("counts");
  assert_eq!((read.in_review, read.done, read.cursor), (3, 4, cursor("9")));
  assert_eq!(
    authority.capabilities().await.expect("capabilities").project_id,
    project
  );
  assert!(
    authority
      .modules(&ModuleListRequest::default())
      .await
      .expect("modules")
      .items
      .is_empty()
  );
  assert!(authority.module_history(module, &PageRequest::first()).await.is_ok());
  assert!(authority.library(&ResourceLibraryRequest::default()).await.is_ok());
  assert!(authority.versions(resource, &PageRequest::first()).await.is_ok());
  let missing = authority.task(task_id).await.expect_err("no such task");
  assert_eq!(missing.code(), Some(AuthorityErrorCode::NotFound));
  let version = VersionNumber::new(std::num::NonZeroU32::new(3).expect("a version"));
  assert!(authority.version(resource, version).await.is_err());
  let lines: Vec<String> = server.seen().into_iter().map(|seen| seen.line).collect();
  let base = format!("/api/v2/projects/{project}");
  assert_eq!(
    lines,
    [
      format!("POST {base}/tasks/counts"),
      format!("GET {base}/capabilities"),
      format!("POST {base}/modules/query"),
      format!("POST {base}/modules/{module}/history"),
      format!("POST {base}/resources/query"),
      format!("POST {base}/resources/{resource}/versions/query"),
      format!("GET {base}/tasks/{task_id}"),
      format!("GET {base}/resources/{resource}/versions/3"),
    ]
  );
}

#[tokio::test]
async fn authority_client_refusals_are_typed() {
  let mut unavailable = Answer::refusal(503, "unavailable");
  unavailable.headers = "Retry-After: 7\r\n";
  let server = Server::start(vec![
    Answer::refusal(410, "cursor_expired"),
    unavailable,
    Answer::status(403, &json!({ "error": "forbidden", "message": "Refused" })),
    Answer::json(&json!({ "items": [] })),
  ]);
  let (client, token) = (server.client(), token());
  let authority = client.authority(&token, ProjectId::generate());
  let request = TaskListRequest::default();
  let expired = authority.tasks(&request).await.expect_err("expired");
  assert!(matches!(&expired, AuthorityCallError::Refused { status: 410, .. }));
  assert_eq!(expired.code(), Some(AuthorityErrorCode::CursorExpired));
  let busy = authority.tasks(&request).await.expect_err("unavailable");
  assert!(matches!(
    busy,
    AuthorityCallError::Refused {
      status: 503,
      retry_after: Some(7),
      ..
    }
  ));
  let legacy = authority
    .tasks(&request)
    .await
    .expect_err("a v1 refusal is not the contract");
  assert_eq!(
    legacy,
    AuthorityCallError::Client(ClientError::NotOrchester { status: 403 })
  );
  let partial = authority.tasks(&request).await.expect_err("a page without its fields");
  assert_eq!(
    partial,
    AuthorityCallError::Client(ClientError::NotOrchester { status: 200 })
  );
}

#[tokio::test]
async fn authority_client_subscription_reads_events_and_sends_the_cursor() {
  let task = TaskId::generate();
  let invalidated =
    json!({"kind": "invalidated", "cursor": "43", "scopes": [{"kind": "task_list"}, {"kind": "task", "task": task}]});
  let server = Server::start(vec![Answer::stream(&[
    ": keep-alive\n\n",
    &sse(&json!({"kind": "ready", "cursor": "42"})),
    &sse(&invalidated),
    &sse(&json!({"kind": "reset", "cursor": "50"})),
    &sse(&json!({"kind": "revoked"})),
    &sse(&json!({"kind": "ready", "cursor": "51"})),
  ])]);
  let (client, token) = (server.client(), token());
  let mut subscription = client
    .authority(&token, ProjectId::generate())
    .subscribe(Some(cursor("40")))
    .await
    .expect("a subscription");
  assert_eq!(
    subscription.next().await,
    Ok(SubscriptionEvent::Ready { cursor: cursor("42") })
  );
  assert_eq!(
    subscription.next().await,
    Ok(SubscriptionEvent::Invalidated {
      cursor: cursor("43"),
      scopes: vec![QueryScope::TaskList {}, QueryScope::Task { task }],
    })
  );
  assert_eq!(
    subscription.next().await,
    Ok(SubscriptionEvent::Reset { cursor: cursor("50") })
  );
  assert_eq!(subscription.next().await, Err(SubscriptionEnd::Revoked));
  assert_eq!(
    subscription.next().await,
    Err(SubscriptionEnd::Revoked),
    "nothing after revoked is read"
  );
  let seen = server.seen();
  assert!(seen[0].line.starts_with("GET /api/v2/projects/") && seen[0].line.ends_with("/subscription"));
  assert_eq!(seen[0].header("last-event-id"), Some("40"));
  assert_eq!(seen[0].header("accept"), Some("text/event-stream"));
}

#[tokio::test]
async fn authority_client_subscription_ends_with_its_reason() {
  let server = Server::start(vec![
    Answer::refusal(401, "unauthenticated"),
    Answer::refusal(403, "membership_revoked"),
    Answer::refusal(404, "not_found"),
    Answer::refusal(503, "unavailable"),
    Answer::stream(&["event: ready\ndata: {\"kind\":\"reset\",\"cursor\":\"3\"}\n\n"]),
    Answer::stream(&[&sse(&json!({"kind": "ready", "cursor": "3"}))]),
  ]);
  let (client, token) = (server.client(), token());
  let authority = client.authority(&token, ProjectId::generate());
  let mut ends = Vec::new();
  for _ in 0..4 {
    ends.push(authority.subscribe(None).await.err());
  }
  assert_eq!(ends[0], Some(SubscriptionEnd::Unauthorized));
  assert_eq!(ends[1], Some(SubscriptionEnd::Revoked));
  assert_eq!(ends[2], Some(SubscriptionEnd::NotFound));
  assert!(
    matches!(&ends[3], Some(SubscriptionEnd::Failed(error)) if error.code() == Some(AuthorityErrorCode::Unavailable))
  );
  let mut mismatched = authority.subscribe(None).await.expect("a stream");
  assert_eq!(
    mismatched.next().await,
    Err(SubscriptionEnd::Unreadable),
    "a name that is not its kind"
  );
  let mut closed = authority.subscribe(None).await.expect("a stream");
  assert!(closed.next().await.is_ok());
  assert_eq!(closed.next().await, Err(SubscriptionEnd::Closed));
}
