use std::{
  io::{BufRead, BufReader, Read, Write},
  net::TcpListener,
  sync::{Arc, Mutex},
  thread,
};

use orchester_api::{
  ErrorCode, FileStored, ResourceCreate, ResourcePatch, ResourcesImport, SharedResourceKind, SharingErrorDetails,
  VersionContent, VersionPublish,
  client::{Client, ClientError, PlainHttp, ServerAddress},
};
use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ResourceId},
  resource::VersionNumber,
  secret::SecretValue,
};
use serde_json::{Value, json};

use crate::fixtures::{hash, key, resource, version};

/// One request as the server read it.
#[derive(Debug)]
pub struct Seen {
  /// e.g. `POST /api/v1/projects/…/resources`.
  pub line: String,
  pub content_type: Option<String>,
  pub body: Vec<u8>,
}

/// A listener on loopback that answers each request with the next of its answers and keeps what it read.
pub struct Server {
  url: String,
  seen: Arc<Mutex<Vec<Seen>>>,
}

impl Server {
  pub fn start(answers: Vec<(u16, Vec<u8>)>) -> Self {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listen");
    let url = format!("http://localhost:{}", listener.local_addr().expect("address").port());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = seen.clone();
    thread::spawn(move || {
      let mut answers = answers.into_iter();
      for mut stream in listener.incoming().flatten() {
        let Some((status, body)) = answers.next() else { return };
        kept.lock().expect("seen").push(read(&stream));
        let head = format!(
          "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
          body.len()
        );
        // The test reads what the client made of the answer; a client that hung up fails there.
        let _ = stream.write_all(head.as_bytes());
        let _ = stream.write_all(&body);
      }
    });
    Self { url, seen }
  }

  pub fn client(&self) -> Client {
    let address = ServerAddress::parse(&self.url).expect("address");
    Client::new(address, PlainHttp::NotAllowed).expect("client")
  }

  pub fn seen(&self) -> Vec<Seen> {
    std::mem::take(&mut *self.seen.lock().expect("seen"))
  }
}

fn read(stream: &std::net::TcpStream) -> Seen {
  let mut reader = BufReader::new(stream);
  let mut line = String::new();
  reader.read_line(&mut line).expect("request line");
  let line = line.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
  let (mut length, mut content_type) = (0, None);
  loop {
    let mut header = String::new();
    reader.read_line(&mut header).expect("header");
    let header = header.trim_end();
    if header.is_empty() {
      break;
    }
    let (name, value) = header.split_once(':').expect("a header has a name");
    match name.to_ascii_lowercase().as_str() {
      "content-length" => length = value.trim().parse().expect("length"),
      "content-type" => content_type = Some(value.trim().to_owned()),
      _ => {}
    }
  }
  let mut body = vec![0; length];
  reader.read_exact(&mut body).expect("body");
  Seen {
    line,
    content_type,
    body,
  }
}

pub fn json_answer(status: u16, value: &Value) -> (u16, Vec<u8>) {
  (status, serde_json::to_vec(value).expect("answer"))
}

pub fn token() -> SecretValue {
  SecretValue::new("token".to_owned()).expect("token")
}

fn content_hash() -> ContentHash {
  ContentHash::new(&hash('e')).expect("hash")
}

#[tokio::test]
async fn resources_the_reads_go_to_their_endpoints() {
  let (project, id) = (ProjectId::generate(), ResourceId::generate());
  let server = Server::start(vec![
    json_answer(200, &json!({"resources":[resource(id, "pipeline")],"cursor":7})),
    json_answer(200, &json!({"versions":[version(id, 1)],"next":1})),
    json_answer(200, &json!({"versions":[version(id, 2)]})),
    (200, b"part".to_vec()),
  ]);
  let client = server.client();
  let resources = client.resources(&token(), project).await.expect("resources");
  assert_eq!((resources.resources.len(), resources.cursor), (1, 7));
  let first = client.versions(&token(), project, id, None).await.expect("first page");
  let next = client
    .versions(&token(), project, id, first.next)
    .await
    .expect("next page");
  assert_eq!(next.versions[0].number, VersionNumber::FIRST.next().expect("v2"));
  let part = client.file(&token(), project, &content_hash()).await.expect("file");
  assert_eq!(part, b"part");
  let lines: Vec<_> = server.seen().into_iter().map(|seen| seen.line).collect();
  let base = format!("/api/v1/projects/{project}");
  assert_eq!(
    lines,
    [
      format!("GET {base}/resources"),
      format!("GET {base}/resources/{id}/versions"),
      format!("GET {base}/resources/{id}/versions?after=1"),
      format!("GET {base}/files/{}", hash('e')),
    ]
  );
}

#[tokio::test]
async fn resources_the_writes_send_their_bodies_to_their_endpoints() {
  let (project, id) = (ProjectId::generate(), ResourceId::generate());
  let server = Server::start(vec![
    json_answer(201, &json!({"resource":resource(id, "tool"),"version":version(id, 1)})),
    json_answer(201, &json!({"resources":[resource(id, "tool")]})),
    json_answer(201, &json!({"version":version(id, 2)})),
    json_answer(200, &json!({"resource":resource(id, "tool")})),
  ]);
  let client = server.client();
  let content = VersionContent {
    body: json!({"name":"Deploy"}),
    references: Vec::new(),
  };
  let create = ResourceCreate {
    idempotency_key: key(),
    id,
    kind: SharedResourceKind::Tool,
    slug: "deploy".to_owned(),
    name: "Deploy".to_owned(),
    version: content.clone(),
  };
  let created = client
    .create_resource(&token(), project, &create)
    .await
    .expect("create");
  assert_eq!(created.version.number, VersionNumber::FIRST);
  let import = ResourcesImport {
    idempotency_key: key(),
    resources: Vec::new(),
  };
  assert_eq!(
    client
      .import_resources(&token(), project, &import)
      .await
      .expect("import")
      .len(),
    1
  );
  let publish = VersionPublish {
    idempotency_key: key(),
    based_on: Some(VersionNumber::FIRST),
    body: content.body,
    references: content.references,
  };
  let published = client
    .publish_version(&token(), project, id, &publish)
    .await
    .expect("publish");
  assert_eq!(published.based_on, Some(VersionNumber::FIRST));
  let patch = ResourcePatch {
    idempotency_key: key(),
    revision: created.resource.revision,
    name: None,
    archived: Some(true),
  };
  client
    .update_resource(&token(), project, id, &patch)
    .await
    .expect("patch");
  let base = format!("/api/v1/projects/{project}");
  let sent: Vec<_> = server
    .seen()
    .into_iter()
    .map(|seen| {
      assert_eq!(seen.content_type.as_deref(), Some("application/json"), "{}", seen.line);
      let body: Value = serde_json::from_slice(&seen.body).expect("a JSON body");
      (seen.line, body)
    })
    .collect();
  let expected = [
    (format!("POST {base}/resources"), serde_json::to_value(&create)),
    (format!("POST {base}/resources/import"), serde_json::to_value(&import)),
    (
      format!("POST {base}/resources/{id}/versions"),
      serde_json::to_value(&publish),
    ),
    (format!("PATCH {base}/resources/{id}"), serde_json::to_value(&patch)),
  ]
  .map(|(line, body)| (line, body.expect("request")));
  assert_eq!(sent, expected);
}

#[tokio::test]
async fn resources_a_file_upload_sends_its_bytes_and_tells_new_from_known() {
  let project = ProjectId::generate();
  let server = Server::start(vec![(201, Vec::new()), (200, Vec::new())]);
  let client = server.client();
  let hash = content_hash();
  let first = client.upload_file(&token(), project, &hash, b"part".to_vec()).await;
  let again = client.upload_file(&token(), project, &hash, b"part".to_vec()).await;
  assert_eq!((first, again), (Ok(FileStored::Created), Ok(FileStored::Known)));
  for seen in server.seen() {
    assert_eq!(seen.line, format!("PUT /api/v1/projects/{project}/files/{hash}"));
    assert_eq!(seen.content_type.as_deref(), Some("application/octet-stream"));
    assert_eq!(seen.body, b"part");
  }
}

#[tokio::test]
async fn resources_a_members_write_is_refused_not_admin_and_a_conflict_carries_the_latest() {
  let (project, id) = (ProjectId::generate(), ResourceId::generate());
  let server = Server::start(vec![
    json_answer(
      403,
      &json!({"error":"not_admin","message":"Only admins change the configuration"}),
    ),
    json_answer(
      409,
      &json!({"error":"version_conflict","message":"Not the latest","latest":version(id, 5)}),
    ),
    json_answer(
      422,
      &json!({"error":"missing_files","message":"Upload the parts","hashes":[hash('f')]}),
    ),
  ]);
  let client = server.client();
  let publish = VersionPublish {
    idempotency_key: key(),
    based_on: Some(VersionNumber::FIRST),
    body: json!({}),
    references: Vec::new(),
  };
  let member = client.publish_version(&token(), project, id, &publish).await;
  assert!(matches!(
    member,
    Err(ClientError::Api {
      status: 403,
      code: ErrorCode::NotAdmin,
      details: None,
      ..
    })
  ));
  let conflict = client
    .publish_version(&token(), project, id, &publish)
    .await
    .expect_err("conflict");
  assert_eq!(conflict.code(), Some(ErrorCode::VersionConflict));
  let Some(SharingErrorDetails::Version { latest }) = conflict.details() else {
    panic!("a conflict names the latest version: {conflict:?}");
  };
  assert_eq!(latest.number.get(), 5);
  let missing = client
    .publish_version(&token(), project, id, &publish)
    .await
    .expect_err("missing files");
  assert_eq!(
    missing.details(),
    Some(&SharingErrorDetails::Files {
      hashes: vec![ContentHash::new(&hash('f')).expect("hash")],
    })
  );
}
