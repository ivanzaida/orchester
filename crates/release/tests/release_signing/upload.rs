//! The upload against the stand-in cloud: the draft, the missing parts only, and the verdict that decides the job.

use orchester_api::releases::{FileResult, NewRelease, Rejection, ReleaseArch, ReleasePlatform, ReleaseSigningKey};
use orchester_core::secret::SecretValue;
use orchester_release::{Draft, PublishError, ReleaseFile, create_draft, finalize, upload_file};
use serde_json::{Value, json};

use super::{SEED, file_name, stand::Stand, version};

const NAME: &str = "Orchester-0.5.0-setup.exe";

fn token() -> SecretValue {
  SecretValue::new("orct_release_0123456789".to_owned()).expect("a token")
}

fn release() -> NewRelease {
  NewRelease {
    version: version("0.5.0"),
    notes: "Orchester 0.5.0".to_owned(),
    critical: false,
    ci_run: None,
  }
}

fn file(bytes: &[u8]) -> ReleaseFile {
  let key = ReleaseSigningKey::parse(SEED).expect("the test key");
  ReleaseFile::sign(
    file_name(NAME),
    bytes.to_vec(),
    &version("0.5.0"),
    ReleasePlatform::Windows,
    ReleaseArch::X86_64,
    &key,
  )
}

fn summary() -> Value {
  json!({ "release": {
    "version": "0.5.0", "stage": "draft", "critical": false, "created_at": 1, "published_at": null, "installs": 0,
    "files_deleted": false,
  }})
}

fn upload(size: u64, received: &[u32]) -> Value {
  json!({ "name": NAME, "size": size, "part_size": 4, "parts": size.div_ceil(4), "received": received })
}

/// A finalize answer with `file`'s result for [`NAME`].
fn verdict(mut file: Value) -> Value {
  file["name"] = json!(NAME);
  json!({ "version": "0.5.0", "files": [file] })
}

#[tokio::test]
async fn release_signing_uploads_only_the_missing_parts_and_finalizes() {
  let stand = Stand::start(vec![
    ("201 Created", summary()),
    ("201 Created", upload(10, &[1])),
    ("200 OK", upload(10, &[0, 1])),
    ("200 OK", upload(10, &[0, 1, 2])),
    ("200 OK", verdict(json!({ "result": "accepted" }))),
  ]);
  let (client, token, release, files) = (&stand.client, &token(), release(), [file(b"0123456789")]);
  assert_eq!(
    create_draft(client, token, &release).await.expect("a draft"),
    Draft::Created
  );
  let sent = upload_file(client, token, &release.version, &files[0])
    .await
    .expect("uploaded");
  assert_eq!(sent, 2);
  let verification = finalize(client, token, &release.version, &files)
    .await
    .expect("accepted");
  assert!(verification.accepted());

  let asked = stand.asked();
  let lines: Vec<&str> = asked.iter().map(|asked| asked.line.as_str()).collect();
  assert_eq!(
    lines,
    [
      "POST /v1/releases".to_owned(),
      "POST /v1/releases/0.5.0/files".to_owned(),
      format!("PUT /v1/releases/0.5.0/files/{NAME}/parts/0"),
      format!("PUT /v1/releases/0.5.0/files/{NAME}/parts/2"),
      "POST /v1/releases/0.5.0/finalize".to_owned(),
    ]
  );
  assert_eq!(
    (asked[2].body.as_slice(), asked[3].body.as_slice()),
    (&b"0123"[..], &b"89"[..])
  );
  for request in &asked {
    assert_eq!(request.authorization.as_deref(), Some("Bearer orct_release_0123456789"));
  }
  let declared: Value = serde_json::from_slice(&asked[1].body).expect("JSON");
  assert_eq!(declared["key_id"], super::KEY_ID);
  assert_eq!(declared["signature"], files[0].declared().signature.to_hex());
  for request in &asked {
    assert!(!String::from_utf8_lossy(&request.body).contains(SEED));
  }
}

#[tokio::test]
async fn release_signing_takes_up_a_draft_an_earlier_run_left() {
  let conflict = json!({ "error": "conflict", "message": "the version exists" });
  let stand = Stand::start(vec![("409 Conflict", conflict)]);
  let draft = create_draft(&stand.client, &token(), &release()).await;
  assert_eq!(draft.expect("taken up"), Draft::Existing);
}

#[tokio::test]
async fn release_signing_fails_unless_the_cloud_accepts_every_file() {
  let rejected = verdict(json!({ "result": "rejected", "reason": "signature_mismatch" }));
  let stand = Stand::start(vec![("200 OK", rejected)]);
  let answer = finalize(&stand.client, &token(), &version("0.5.0"), &[file(b"0123")]).await;
  let Err(PublishError::Rejected(verdicts)) = answer else {
    panic!("rejected, not {answer:?}");
  };
  assert_eq!(
    verdicts[0].result,
    FileResult::Rejected {
      reason: Rejection::SignatureMismatch
    }
  );

  let other =
    json!({ "version": "0.5.0", "files": [{ "name": "Orchester-0.5.0-windows-x64.zip", "result": "accepted" }] });
  let stand = Stand::start(vec![("200 OK", other)]);
  let answer = finalize(&stand.client, &token(), &version("0.5.0"), &[file(b"0123")]).await;
  assert!(matches!(answer, Err(PublishError::Rejected(_))));
}

#[tokio::test]
async fn release_signing_refuses_an_upload_that_does_not_fit_the_file() {
  let stand = Stand::start(vec![("201 Created", upload(11, &[]))]);
  let answer = upload_file(&stand.client, &token(), &version("0.5.0"), &file(b"0123456789")).await;
  assert!(matches!(answer, Err(PublishError::UnexpectedUpload { .. })));
  assert_eq!(stand.asked().len(), 1);
}

#[tokio::test]
async fn release_signing_fails_when_parts_are_still_missing() {
  let stand = Stand::start(vec![
    ("201 Created", upload(8, &[])),
    ("200 OK", upload(8, &[0])),
    ("200 OK", upload(8, &[0])),
  ]);
  let answer = upload_file(&stand.client, &token(), &version("0.5.0"), &file(b"01234567")).await;
  let Err(PublishError::PartsMissing { missing, .. }) = answer else {
    panic!("parts missing, not {answer:?}");
  };
  assert_eq!(missing, [1]);
}
