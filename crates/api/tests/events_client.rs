//! `Client::events` against a fake server on loopback (`docs/architecture/server.md`, "Live updates"): typed snapshot
//! pages and deltas, the cursor sent as `Last-Event-ID`, keep-alives skipped, and `401`, `403` and `revoked` ending the
//! stream with their reason.
//!
//! `cargo test -p orchester-api --features client --test events_client live_updates_`

use std::{
  io::{BufRead, BufReader, Write},
  net::{TcpListener, TcpStream},
  sync::{Arc, Mutex},
  thread,
  time::Duration,
};

use orchester_api::{
  ClaimAction, Event,
  client::{Client, ClientError, PlainHttp, ServerAddress, StreamEnd},
};
use orchester_core::{id::ProjectId, secret::SecretValue};
use serde_json::{Value, json};

const TASK: &str = "01a183cf-b73c-71db-8a62-bd509770cd4d";
const USER: &str = "01a183cf-b73c-71db-8a62-bd509770cd4e";
const PROJECT: &str = "01a183cf-b73c-71db-8a62-bd509770cd52";

/// One answer: its status and content type, and its body in the pieces it is written in.
struct Answer {
  status: u16,
  content_type: &'static str,
  pieces: Vec<String>,
}

impl Answer {
  fn stream(pieces: &[&str]) -> Self {
    Self {
      status: 200,
      content_type: "text/event-stream; charset=utf-8",
      pieces: pieces.iter().map(|piece| (*piece).to_owned()).collect(),
    }
  }

  fn refusal(status: u16, error: &str) -> Self {
    Self {
      status,
      content_type: "application/json",
      pieces: vec![json!({ "error": error, "message": "Refused" }).to_string()],
    }
  }
}

/// The request line and headers, lower-cased names, of one request the server read.
type Seen = (String, Vec<(String, String)>);

/// A listener on loopback that answers each connection with the next of its answers, writing the body piece by piece
/// and closing the connection after it, and keeps what it read.
struct Server {
  url: String,
  seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
  fn start(answers: Vec<Answer>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let url = format!("http://localhost:{}", listener.local_addr().expect("address").port());
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
    let address = ServerAddress::parse(&self.url).expect("address");
    Client::new(address, PlainHttp::NotAllowed).expect("client")
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
      return (line, headers);
    }
    let (name, value) = header.split_once(':').expect("a header has a name");
    headers.push((name.to_ascii_lowercase(), value.trim().to_owned()));
  }
}

fn write(stream: &mut TcpStream, answer: &Answer) -> std::io::Result<()> {
  let head = format!(
    "HTTP/1.1 {} Answer\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
    answer.status, answer.content_type
  );
  stream.write_all(head.as_bytes())?;
  for piece in &answer.pieces {
    stream.write_all(piece.as_bytes())?;
    stream.flush()?;
    // Apart, so the client reads them as separate chunks.
    thread::sleep(Duration::from_millis(20));
  }
  Ok(())
}

fn header<'a>(seen: &'a Seen, name: &str) -> Option<&'a str> {
  seen
    .1
    .iter()
    .find(|(header, _)| header == name)
    .map(|(_, value)| value.as_str())
}

fn token() -> SecretValue {
  SecretValue::new("token".to_owned()).expect("token")
}

fn sse(kind: &str, data: &Value) -> String {
  format!("event: {kind}\ndata: {data}\n\n")
}

fn alice() -> Value {
  json!({ "id": USER, "display_name": "Alice" })
}

fn task() -> Value {
  let attribution = json!({ "by": alice(), "at": 1 });
  json!({
    "id": TASK, "number": 108, "key": "ORC-108", "former_key": null, "revision": 4, "title": "Status bar",
    "brief": "", "criteria": [], "pipeline": null, "limits": null, "workflow": "in_progress", "priority": "high",
    "tags": [], "blocking": [], "blocked_reason": null, "module": null, "status": "queued", "has_run": false,
    "created": attribution, "updated": attribution, "claim": null, "run": null
  })
}

fn snapshot_begin() -> Value {
  json!({
    "project": { "id": PROJECT, "slug": "orchester", "address": "https://team.example/orchester", "key": "ORC",
      "name": "Orchester", "remote_hint": null, "role": "member", "next_number": 400 },
    "members": [], "tags": [], "modules": []
  })
}

fn claim_released(id: u64) -> String {
  let data = json!({ "id": id, "by": alice(), "actor": "person", "at": 1_700, "task": TASK, "claim": null,
    "action": "released" });
  format!("id: {id}\n{}", sse("task.claim", &data))
}

#[tokio::test]
async fn live_updates_a_stream_without_cursor_yields_the_snapshot_then_deltas() {
  let delta = claim_released(42);
  let (head, tail) = delta.split_at(delta.len() / 2);
  let server = Server::start(vec![Answer::stream(&[
    ": keep-alive\n\n",
    &sse("snapshot.begin", &snapshot_begin()),
    &sse("snapshot.tasks", &json!({ "tasks": [task()] })),
    &format!("{}: keep-alive\n\n", sse("snapshot.end", &json!({ "cursor": 41 }))),
    head,
    tail,
  ])]);
  let project = ProjectId::generate();
  let mut stream = server.client().events(&token(), project, None).await.expect("a stream");
  let Ok(Event::SnapshotBegin(begin)) = stream.next().await else {
    panic!("the snapshot's first page");
  };
  assert_eq!(begin.project.key, "ORC");
  let Ok(Event::SnapshotTasks(page)) = stream.next().await else {
    panic!("a page of tasks");
  };
  assert_eq!(page.tasks.len(), 1);
  let Ok(Event::SnapshotEnd(end)) = stream.next().await else {
    panic!("the snapshot's end");
  };
  assert_eq!(end.cursor, 41);
  let Ok(Event::TaskClaim(released)) = stream.next().await else {
    panic!("a claim delta, split over two chunks");
  };
  assert_eq!((released.id, released.change.action), (42, ClaimAction::Released));
  assert_eq!(stream.next().await.err(), Some(StreamEnd::Closed));
  assert_eq!(
    stream.next().await.err(),
    Some(StreamEnd::Closed),
    "an ended stream says why again"
  );
  let seen = server.seen();
  assert_eq!(seen[0].0, format!("GET /api/v1/projects/{project}/events"));
  assert_eq!(header(&seen[0], "accept"), Some("text/event-stream"));
  assert_eq!(header(&seen[0], "authorization"), Some("Bearer token"));
  assert_eq!(header(&seen[0], "last-event-id"), None, "no cursor, no Last-Event-ID");
}

#[tokio::test]
async fn live_updates_a_cursor_is_sent_and_revoked_ends_the_stream() {
  let server = Server::start(vec![Answer::stream(&[
    &claim_released(42),
    &sse("revoked", &json!({})),
    &claim_released(43),
  ])]);
  let mut stream = server
    .client()
    .events(&token(), ProjectId::generate(), Some(41))
    .await
    .expect("a stream");
  assert!(matches!(stream.next().await, Ok(Event::TaskClaim(delta)) if delta.id == 42));
  assert_eq!(stream.next().await.err(), Some(StreamEnd::Revoked));
  assert_eq!(
    stream.next().await.err(),
    Some(StreamEnd::Revoked),
    "nothing after revoked is read"
  );
  assert_eq!(header(&server.seen()[0], "last-event-id"), Some("41"));
}

#[tokio::test]
async fn live_updates_a_refused_stream_ends_with_its_reason() {
  let server = Server::start(vec![
    Answer::refusal(401, "unauthorized"),
    Answer::refusal(403, "forbidden"),
    Answer::refusal(429, "rate_limited"),
    Answer {
      status: 200,
      content_type: "text/html",
      pieces: vec!["<html></html>".to_owned()],
    },
  ]);
  let client = server.client();
  let mut ends = Vec::new();
  for _ in 0..4 {
    ends.push(client.events(&token(), ProjectId::generate(), None).await.err());
  }
  assert_eq!(ends[0], Some(StreamEnd::Unauthorized));
  assert_eq!(ends[1], Some(StreamEnd::Forbidden));
  assert!(matches!(
    &ends[2],
    Some(StreamEnd::Failed(error)) if error.code() == Some(orchester_api::ErrorCode::RateLimited)
  ));
  assert_eq!(
    ends[3],
    Some(StreamEnd::Failed(ClientError::NotOrchester { status: 200 }))
  );
}

#[tokio::test]
async fn live_updates_an_unknown_kind_ends_the_stream_unreadable() {
  let server = Server::start(vec![Answer::stream(&[&sse("reset", &json!({})), &claim_released(42)])]);
  let mut stream = server
    .client()
    .events(&token(), ProjectId::generate(), None)
    .await
    .expect("a stream");
  assert_eq!(stream.next().await.err(), Some(StreamEnd::Unreadable));
}
