use std::time::Duration;

use orchester_api::Credential;
use orchester_server::clock::Clock;
use serde_json::json;
use tokio::{
  io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
  net::TcpStream,
};

use crate::{fixture::*, support::TestServer};

pub(super) struct Stream {
  socket: BufReader<TcpStream>,
  pending: String,
}
pub(super) struct Frame {
  pub id: Option<u64>,
  pub kind: Option<String>,
  pub data: String,
  comment: bool,
}
impl Stream {
  pub(super) async fn open(
    server: &TestServer,
    token: &Credential,
    project: orchester_core::id::ProjectId,
    after: u64,
  ) -> Self {
    Self::try_open(server, token, project, after)
      .await
      .unwrap_or_else(|status| panic!("expected SSE success, got {status}"))
  }
  pub(super) async fn try_open(
    server: &TestServer,
    token: &Credential,
    project: orchester_core::id::ProjectId,
    after: u64,
  ) -> Result<Self, u16> {
    let mut socket = server.connect().await;
    socket.write_all(format!("GET /api/v1/projects/{project}/events HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nLast-Event-ID: {after}\r\n\r\n",token.expose()).as_bytes()).await.expect("subscribe");
    let mut socket = BufReader::new(socket);
    let mut line = String::new();
    socket.read_line(&mut line).await.expect("status");
    let status = line
      .split_whitespace()
      .nth(1)
      .expect("status code")
      .parse()
      .expect("numeric status");
    if status != 200 {
      return Err(status);
    }
    let mut buffering = false;
    loop {
      line.clear();
      socket.read_line(&mut line).await.expect("headers");
      if line == "\r\n" {
        break;
      }
      if line.eq_ignore_ascii_case("x-accel-buffering: no\r\n") {
        buffering = true;
      }
    }
    assert!(buffering);
    Ok(Self {
      socket,
      pending: String::new(),
    })
  }
  async fn frame(&mut self) -> Option<Frame> {
    loop {
      if let Some(end) = self.pending.find("\n\n") {
        let text: String = self.pending.drain(..end + 2).collect();
        let mut frame = Frame {
          id: None,
          kind: None,
          data: String::new(),
          comment: text.starts_with(':'),
        };
        for line in text.lines() {
          if let Some(id) = line.strip_prefix("id: ") {
            frame.id = Some(id.parse().expect("event id"));
          }
          if let Some(kind) = line.strip_prefix("event: ") {
            frame.kind = Some(kind.into());
          }
          if let Some(data) = line.strip_prefix("data: ") {
            frame.data.push_str(data);
          }
        }
        return Some(frame);
      }
      let mut size = String::new();
      let read = self.socket.read_line(&mut size).await.expect("chunk size");
      if read == 0 {
        return None;
      }
      let size = usize::from_str_radix(size.trim().split(';').next().expect("size"), 16).expect("hex chunk");
      if size == 0 {
        return None;
      }
      let mut bytes = vec![0; size];
      self.socket.read_exact(&mut bytes).await.expect("chunk");
      let mut crlf = [0; 2];
      self.socket.read_exact(&mut crlf).await.expect("chunk delimiter");
      assert_eq!(&crlf, b"\r\n");
      self.pending.push_str(std::str::from_utf8(&bytes).expect("UTF8 events"));
    }
  }
  pub(super) async fn next(&mut self) -> Frame {
    tokio::time::timeout(Duration::from_secs(5), async {
      loop {
        let frame = self.frame().await.expect("open stream");
        if !frame.comment {
          return frame;
        }
      }
    })
    .await
    .expect("event deadline")
  }
  pub(super) async fn closed(&mut self) {
    assert!(
      tokio::time::timeout(Duration::from_secs(5), self.frame())
        .await
        .expect("close deadline")
        .is_none()
    );
  }
}

#[tokio::test]
async fn s1b_sse_resumes_exact_snapshots_and_replays_more_than_one_page() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  let tasks = create_tasks(&server, &owner.token, project, 105).await.tasks;
  let edited = changed(
    &patch(
      &server,
      &owner.token,
      project,
      &tasks[0],
      json!({"title":"Edited after creation"}),
    )
    .await,
  );
  let mut stream = Stream::open(&server, &owner.token, project, created.cursor).await;
  let mut last = created.cursor;
  for original in &tasks {
    let event = stream.next().await;
    assert_eq!(event.kind.as_deref(), Some("task"));
    let id = event.id.expect("cursor");
    assert!(id > last);
    last = id;
    let task: orchester_api::SharedTask = serde_json::from_str(&event.data).expect("task snapshot");
    assert_eq!(task.id, original.id);
    assert_eq!(task.content.title, original.content.title);
  }
  let event = stream.next().await;
  assert_eq!(event.kind.as_deref(), Some("task"));
  let last = event.id.expect("edited cursor");
  assert_eq!(
    serde_json::from_str::<orchester_api::SharedTask>(&event.data)
      .expect("edited")
      .content
      .title,
    edited.content.title
  );
  drop(stream);
  let mut resumed = Stream::open(&server, &owner.token, project, last).await;
  let next = tag(&server, &owner.token, project, "Only missed event").await;
  let event = resumed.next().await;
  assert_eq!(event.kind.as_deref(), Some("tags"));
  assert!(event.data.contains(&next.id.to_string()));
  assert!(event.id.expect("tag cursor") > last);
  drop(resumed);
  server.finish().await;
}

#[tokio::test]
async fn s1b_sse_retention_reset_at_explicit_clock_boundary() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  create_tasks(&server, &owner.token, project, 1).await;
  server.clock.advance(30 * 24 * 60 * 60 * 1000);
  let fresh = server.another_device(&owner.token).await;
  let mut boundary = Stream::open(&server, &fresh.token, project, 0).await;
  assert_ne!(boundary.next().await.kind.as_deref(), Some("reset"));
  drop(boundary);
  server.clock.advance(1);
  let mut reset = Stream::open(&server, &fresh.token, project, 0).await;
  assert_eq!(reset.next().await.kind.as_deref(), Some("reset"));
  reset.closed().await;
  let task = create_tasks(&server, &fresh.token, project, 1).await.tasks.remove(0);
  assert_eq!(task.number, 2);
  let removed: i64 = server
    .database()
    .query_row(
      "SELECT COUNT(*) FROM events WHERE at < ?1",
      [server.clock.now_ms() - 30 * 24 * 60 * 60 * 1000],
      |row| row.get(0),
    )
    .expect("retention");
  assert_eq!(removed, 0);
  let mut reset_again = Stream::open(&server, &fresh.token, project, 0).await;
  assert_eq!(reset_again.next().await.kind.as_deref(), Some("reset"));
  reset_again.closed().await;
  server.finish().await;
}

#[tokio::test]
async fn s1b_sse_session_revocation_and_membership_lifetime_close_streams() {
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let bob = server.invited("bob", false).await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  add_member(&server, &owner.token, project, "bob", "member").await;
  let cursor = server
    .request("GET", &format!("{}/tasks", path(project)), Some(&bob.token), None)
    .await
    .json::<orchester_api::Tasks>()
    .cursor;
  let mut stream = Stream::open(&server, &bob.token, project, cursor).await;
  assert_eq!(
    server
      .request(
        "DELETE",
        &format!("{}/members/{}", path(project), bob.user.id),
        Some(&owner.token),
        None
      )
      .await
      .status,
    204
  );
  add_member(&server, &owner.token, project, "bob", "member").await;
  assert_eq!(stream.next().await.kind.as_deref(), Some("revoked"));
  stream.closed().await;
  let cursor = server
    .request("GET", &format!("{}/tasks", path(project)), Some(&bob.token), None)
    .await
    .json::<orchester_api::Tasks>()
    .cursor;
  let mut stream = Stream::open(&server, &bob.token, project, cursor).await;
  assert_eq!(
    server
      .request("DELETE", "/auth/sessions/current", Some(&bob.token), None)
      .await
      .status,
    204
  );
  assert_eq!(stream.next().await.kind.as_deref(), Some("revoked"));
  stream.closed().await;
  server.finish().await;
}

#[tokio::test]
async fn s1b_sse_keepalive_checks_idle_expiry_and_server_shutdown() {
  use orchester_server::clock::Clock;
  let server = TestServer::start(|_| {}).await;
  let owner = server.bootstrap().await;
  let created = project(&server, &owner.token, vec![]).await;
  let project = created.project.id;
  let mut stream = Stream::open(&server, &owner.token, project, created.cursor).await;
  assert!(stream.frame().await.expect("initial comment").comment);
  let keepalive = tokio::time::timeout(Duration::from_secs(23), stream.frame())
    .await
    .expect("20-second keepalive")
    .expect("comment");
  assert!(keepalive.comment);
  let persisted: i64 = server
    .database()
    .query_row(
      "SELECT last_used_at FROM sessions WHERE id=?1",
      [owner.session.id.to_string()],
      |row| row.get(0),
    )
    .expect("session timestamp");
  server
    .clock
    .advance(persisted + orchester_server::clock::DEVICE_IDLE_MS - server.clock.now_ms());
  let event = tokio::time::timeout(Duration::from_secs(23), async {
    loop {
      let frame = stream.frame().await.expect("expiry event");
      if !frame.comment {
        return frame;
      }
    }
  })
  .await
  .expect("expiry on keepalive");
  assert_eq!(event.kind.as_deref(), Some("revoked"));
  stream.closed().await;
  let code = server
    .server
    .host_account_code("alice".into())
    .await
    .expect("account code");
  let fresh = server.join_as(&code, "alice", "rejoined laptop").await;
  let mut open = Stream::open(&server, &fresh.token, project, created.cursor).await;
  assert!(open.frame().await.expect("initial").comment);
  server.finish().await;
  open.closed().await;
}
