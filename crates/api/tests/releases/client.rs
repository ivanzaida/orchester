//! The client's release calls against the stand-in cloud: what each sends, and how it reads the answers.

use orchester_api::{
  client::ClientError,
  cloud::CloudErrorCode,
  releases::{
    Channel, DeclaredFile, FileDigest, FileSignature, NewRelease, ReleaseArch, ReleaseChanges, ReleaseKeyId,
    ReleasePlatform, ReleaseStage, RolloutPercent, UpdateCheck,
  },
};
use orchester_core::secret::SecretValue;
use serde_json::{Value, json};

use super::{
  DIGEST, KEY_ID, SIGNATURE, file_name,
  stand::{Answer, Stand},
  version,
};

const INSTALL: &str = "0192f4d5-7c3a-7b2e-9a41-5d6c7e8f9a0b";
const NAME: &str = "Orchester-0.4.1-windows-x86_64.msi";

fn token() -> SecretValue {
  SecretValue::new("orct_0123456789abcdef".to_owned()).expect("a token")
}

fn check() -> UpdateCheck {
  UpdateCheck {
    version: version("0.4.0"),
    platform: ReleasePlatform::Windows,
    arch: ReleaseArch::X86_64,
    channel: Channel::Stable,
    install_id: INSTALL.parse().expect("an id"),
  }
}

fn summary(stage: &str) -> Value {
  json!({ "release": {
    "version": "0.4.1", "stage": stage, "critical": false, "created_at": 1, "published_at": null, "installs": 0,
    "files_deleted": false,
  }})
}

fn upload(received: &[u32]) -> Value {
  json!({ "name": NAME, "size": 10, "part_size": 8, "parts": 2, "received": received })
}

#[tokio::test]
async fn releases_client_checks_for_an_update_without_signing_in() {
  let stand = Stand::start(vec![Answer::json("200 OK", &json!({ "update": null }))]);
  assert_eq!(stand.client.check_for_update(&check()).await, Ok(None));
  let asked = stand.asked();
  assert_eq!(
    asked[0].line,
    format!("GET /v1/updates/check?version=0.4.0&platform=windows&arch=x86_64&channel=stable&install_id={INSTALL}")
  );
  assert_eq!(asked[0].header("authorization"), None);
}

#[tokio::test]
async fn releases_client_reads_the_offered_release() {
  let offer = json!({ "update": {
    "version": "0.4.1", "channel": "stable", "notes": "Fixes", "critical": false, "published_at": 5,
    "files": [{ "name": NAME, "platform": "windows", "arch": "x86_64", "url": "https://x/f", "size": 10,
      "sha256": DIGEST, "key_id": KEY_ID, "signature": SIGNATURE }],
  }});
  let stand = Stand::start(vec![Answer::json("200 OK", &offer)]);
  let offered = stand
    .client
    .check_for_update(&check())
    .await
    .expect("an answer")
    .expect("an offer");
  assert_eq!(offered.version, version("0.4.1"));
  assert_eq!(offered.files[0].key_id, ReleaseKeyId::parse(KEY_ID).expect("a key id"));
}

#[tokio::test]
async fn releases_client_resumes_a_download_with_a_range() {
  let answer = Answer {
    status: "206 Partial Content",
    headers: vec!["Content-Range: bytes 4-9/10".to_owned()],
    body: b"456789".to_vec(),
  };
  let stand = Stand::start(vec![answer]);
  let mut download = stand
    .client
    .download(&version("0.4.1"), &file_name(NAME), 4)
    .await
    .expect("a download");
  assert_eq!((download.start(), download.total()), (4, 10));
  let mut bytes = Vec::new();
  while let Some(chunk) = download.chunk().await.expect("a chunk") {
    bytes.extend(chunk);
  }
  assert_eq!(bytes, b"456789");
  let asked = stand.asked();
  assert_eq!(asked[0].line, format!("GET /v1/releases/0.4.1/files/{NAME}"));
  assert_eq!(asked[0].header("range"), Some("bytes=4-"));
  assert_eq!(asked[0].header("authorization"), None);
}

#[tokio::test]
async fn releases_client_downloads_the_whole_file_from_the_start() {
  let answer = Answer {
    status: "200 OK",
    headers: vec!["Accept-Ranges: bytes".to_owned()],
    body: b"0123456789".to_vec(),
  };
  let stand = Stand::start(vec![answer]);
  let download = stand
    .client
    .download(&version("0.4.1"), &file_name(NAME), 0)
    .await
    .expect("a download");
  assert_eq!((download.start(), download.total()), (0, 10));
  assert_eq!(stand.asked()[0].header("range"), None);
}

#[tokio::test]
async fn releases_client_reads_a_range_past_the_end_as_the_clouds_refusal() {
  let refusal = json!({ "error": "range_not_satisfiable", "message": "past the end" });
  let stand = Stand::start(vec![Answer::json("416 Range Not Satisfiable", &refusal)]);
  let answer = stand.client.download(&version("0.4.1"), &file_name(NAME), 10).await;
  assert!(matches!(
    answer,
    Err(ClientError::Cloud {
      status: 416,
      code: CloudErrorCode::RangeNotSatisfiable,
      ..
    })
  ));
}

#[tokio::test]
async fn releases_client_uploads_a_release_in_parts_and_finalizes_it() {
  let verdict = json!({ "version": "0.4.1", "files": [{ "name": NAME, "result": "accepted" }] });
  let stand = Stand::start(vec![
    Answer::json("201 Created", &summary("draft")),
    Answer::json("201 Created", &upload(&[])),
    Answer::json("200 OK", &upload(&[1])),
    Answer::json("200 OK", &verdict),
  ]);
  let client = &stand.client;
  let release = NewRelease {
    version: version("0.4.1"),
    notes: "Fixes".to_owned(),
    critical: false,
  };
  let created = client.create_release(&token(), &release).await.expect("created");
  assert_eq!(created.stage, ReleaseStage::Draft);
  let file = DeclaredFile {
    name: file_name(NAME),
    platform: ReleasePlatform::Windows,
    arch: ReleaseArch::X86_64,
    size: 10,
    sha256: FileDigest::parse(DIGEST).expect("a digest"),
    key_id: ReleaseKeyId::parse(KEY_ID).expect("a key id"),
    signature: FileSignature::parse(SIGNATURE).expect("a signature"),
  };
  let declared = client
    .declare_file(&token(), &release.version, &file)
    .await
    .expect("declared");
  assert_eq!(declared.missing(), [0, 1]);
  let sent = client
    .upload_part(&token(), &release.version, &file.name, 1, b"89".to_vec())
    .await
    .expect("a part");
  assert_eq!(sent.missing(), [0]);
  let verification = client
    .finalize_release(&token(), &release.version)
    .await
    .expect("finalized");
  assert!(verification.accepted());

  let asked = stand.asked();
  let lines: Vec<&str> = asked.iter().map(|asked| asked.line.as_str()).collect();
  assert_eq!(
    lines,
    [
      "POST /v1/releases".to_owned(),
      "POST /v1/releases/0.4.1/files".to_owned(),
      format!("PUT /v1/releases/0.4.1/files/{NAME}/parts/1"),
      "POST /v1/releases/0.4.1/finalize".to_owned(),
    ]
  );
  for request in &asked {
    assert_eq!(request.header("authorization"), Some("Bearer orct_0123456789abcdef"));
  }
  assert_eq!(asked[2].header("content-type"), Some("application/octet-stream"));
  assert_eq!(asked[2].body, b"89");
  let body: Value = serde_json::from_slice(&asked[1].body).expect("JSON");
  assert_eq!(body["signature"], SIGNATURE);
}

#[tokio::test]
async fn releases_client_reads_a_token_without_the_scope_as_forbidden() {
  let refusal = json!({ "error": "forbidden", "message": "releases:write" });
  let stand = Stand::start(vec![Answer::json("403 Forbidden", &refusal)]);
  let answer = stand.client.finalize_release(&token(), &version("0.4.1")).await;
  assert!(matches!(
    answer,
    Err(ClientError::Cloud {
      status: 403,
      code: CloudErrorCode::Forbidden,
      ..
    })
  ));
}

#[tokio::test]
async fn releases_client_sends_each_admin_call_to_its_endpoint() {
  let releases = json!({ "releases": [summary("beta")["release"]] });
  let mut details = summary("beta");
  let fields = details["release"].as_object_mut().expect("an object");
  for (key, value) in [
    ("notes", json!("")),
    ("files", json!([])),
    ("adoption", json!([])),
    ("activity", json!([])),
  ] {
    fields.insert(key.to_owned(), value);
  }
  let stand = Stand::start(vec![
    Answer::json("200 OK", &releases),
    Answer::json("200 OK", &details),
    Answer::json("200 OK", &summary("beta")),
    Answer::json("200 OK", &summary("beta")),
    Answer::json("200 OK", &summary("beta")),
    Answer::json("200 OK", &summary("beta")),
    Answer::json("200 OK", &summary("withdrawn")),
    Answer::json("200 OK", &summary("beta")),
    Answer::empty("204 No Content"),
  ]);
  let (client, device, release) = (&stand.client, &token(), &version("0.4.1"));
  let ten = RolloutPercent::new(10).expect("a percentage");
  assert_eq!(client.admin_releases(device).await.expect("listed").len(), 1);
  assert_eq!(client.admin_release(device, release).await.expect("read").notes, "");
  client.promote_release(device, release, ten).await.expect("promoted");
  client.set_rollout(device, release, ten).await.expect("set");
  client.pause_release(device, release).await.expect("paused");
  client.resume_release(device, release).await.expect("resumed");
  let withdrawn = client.withdraw_release(device, release).await.expect("withdrawn");
  assert_eq!(withdrawn.stage, ReleaseStage::Withdrawn);
  let changes = ReleaseChanges {
    notes: Some("New notes".to_owned()),
    critical: None,
  };
  client.change_release(device, release, &changes).await.expect("changed");
  client.delete_release_files(device, release).await.expect("deleted");

  assert_eq!(
    stand.lines(),
    [
      "GET /v1/admin/releases",
      "GET /v1/admin/releases/0.4.1",
      "POST /v1/admin/releases/0.4.1/promote",
      "PUT /v1/admin/releases/0.4.1/rollout",
      "POST /v1/admin/releases/0.4.1/pause",
      "POST /v1/admin/releases/0.4.1/resume",
      "POST /v1/admin/releases/0.4.1/withdraw",
      "PATCH /v1/admin/releases/0.4.1",
      "DELETE /v1/admin/releases/0.4.1/files",
    ]
  );
  let asked = stand.asked();
  assert_eq!(asked[2].body, br#"{"percent":10}"#);
  assert_eq!(asked[7].body, br#"{"notes":"New notes"}"#);
}
