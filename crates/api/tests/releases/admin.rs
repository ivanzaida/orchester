//! The JSON of the admin's release shapes, written and read back, and the shapes of a cloud before ORC-575 read as
//! they were.

use orchester_api::releases::{
  AdminFile, AdoptionDay, Channel, ChannelInstalls, CiRun, CiRunUrl, FileDigest, FileProgress, NewRelease,
  PercentChange, Rejection, ReleaseAction, ReleaseActivity, ReleaseArch, ReleaseChanges, ReleaseDetails,
  ReleaseFormatError, ReleasePlatform, ReleaseStage, ReleaseSummary, ReleasesResponse, RolloutPercent, UploadState,
  Withdrawal,
};
use serde_json::{Value, json};

use super::{
  DIGEST, file_name,
  shapes::{NAME, digest, key_id, round_trip},
  version,
};

const RUN_URL: &str = "https://github.com/ivanzaida/orchester/actions/runs/123456";

fn percent(value: u8) -> RolloutPercent {
  RolloutPercent::new(value).expect("a percentage")
}

fn summary(stage: ReleaseStage) -> ReleaseSummary {
  ReleaseSummary {
    version: version("0.4.1"),
    stage,
    critical: false,
    created_at: 1_792_000_000_000,
    published_at: Some(1_792_100_000_000),
    installs: 1_204,
    installs_by_channel: None,
    files_deleted: false,
    ci_run: None,
    withdrawn: None,
    uploads: Vec::new(),
  }
}

/// `fields` added to (or replacing those of) `value`, both JSON objects.
fn with(mut value: Value, fields: Value) -> Value {
  let Value::Object(fields) = fields else {
    panic!("fields are a JSON object");
  };
  value.as_object_mut().expect("an object").extend(fields);
  value
}

/// [`summary`]'s JSON with the stage's `fields`.
fn summary_json(stage: Value) -> Value {
  let value = json!({
    "version": "0.4.1", "critical": false, "created_at": 1_792_000_000_000_i64,
    "published_at": 1_792_100_000_000_i64, "installs": 1_204, "installs_by_channel": null, "files_deleted": false,
    "ci_run": null, "withdrawn": null, "uploads": [],
  });
  with(value, stage)
}

#[test]
fn releases_stages_carry_their_rollout_and_pause() {
  let stable = summary(ReleaseStage::Stable {
    percent: percent(25),
    paused: true,
  });
  round_trip(
    &stable,
    &summary_json(json!({ "stage": "stable", "percent": 25, "paused": true })),
  );
  assert_eq!(stable.stage.channels(), [Channel::Stable, Channel::Beta]);
  let beta = summary(ReleaseStage::Beta { paused: true });
  round_trip(&beta, &summary_json(json!({ "stage": "beta", "paused": true })));
  assert_eq!(beta.stage.channels(), [Channel::Beta]);
  assert!(ReleaseStage::Withdrawn.channels().is_empty());
  assert!(ReleaseStage::Draft.channels().is_empty());
}

#[test]
fn releases_old_admin_payloads_still_parse() {
  let old = json!({
    "version": "0.4.1", "stage": "beta", "critical": false, "created_at": 1_792_000_000_000_i64,
    "published_at": 1_792_100_000_000_i64, "installs": 1_204, "files_deleted": false,
  });
  let read: ReleaseSummary = serde_json::from_value(old.clone()).expect("an old summary");
  assert_eq!(read, summary(ReleaseStage::Beta { paused: false }));
  let list: ReleasesResponse = serde_json::from_value(json!({ "releases": [old] })).expect("an old list");
  assert_eq!(list.installs_by_channel, None);
  let day: AdoptionDay = serde_json::from_value(json!({ "day": "2026-10-29", "installs": 3 })).expect("an old day");
  assert_eq!(day.installs_by_channel, None);
  for (action, expected) in [
    (
      "paused",
      ReleaseAction::Paused {
        channel: Channel::Stable,
      },
    ),
    (
      "resumed",
      ReleaseAction::Resumed {
        channel: Channel::Stable,
      },
    ),
  ] {
    let entry = json!({ "at": 1, "actor": "Ivan", "action": action });
    let read: ReleaseActivity = serde_json::from_value(entry).expect("an old entry");
    assert_eq!(read.action, expected);
  }
}

#[test]
fn releases_new_release_names_its_ci_run_only_when_it_has_one() {
  let created = json!({ "version": "0.4.1", "notes": "", "critical": false });
  let release: NewRelease = serde_json::from_value(created.clone()).expect("an old new release");
  assert_eq!(release.ci_run, None);
  assert_eq!(serde_json::to_value(&release).expect("written"), created);
  let named = NewRelease {
    ci_run: Some(CiRun {
      number: 48,
      url: CiRunUrl::parse(RUN_URL).expect("a link"),
    }),
    ..release
  };
  round_trip(
    &named,
    &with(created, json!({ "ci_run": { "number": 48, "url": RUN_URL } })),
  );
}

#[test]
fn releases_ci_run_link_is_https() {
  for link in [
    "http://ci.example.com/run/1",
    "https://",
    "ftp://ci.example.com",
    "https://ci.example.com/a b",
    "javascript:alert(1)",
  ] {
    assert_eq!(CiRunUrl::parse(link), Err(ReleaseFormatError::CiRunUrl), "{link}");
  }
  let long = format!("https://ci.example.com/{}", "a".repeat(2048));
  assert_eq!(CiRunUrl::parse(&long), Err(ReleaseFormatError::CiRunUrl));
  let run = json!({ "number": 1, "url": "http://ci.example.com/run/1" });
  assert!(serde_json::from_value::<CiRun>(run).is_err());
}

#[test]
fn releases_list_carries_installs_by_channel() {
  let everyone = ChannelInstalls { stable: 414, beta: 21 };
  assert_eq!(everyone.total(), 435);
  let list = ReleasesResponse {
    releases: vec![ReleaseSummary {
      installs: 142,
      installs_by_channel: Some(ChannelInstalls { stable: 96, beta: 46 }),
      ..summary(ReleaseStage::Beta { paused: false })
    }],
    installs_by_channel: Some(everyone),
  };
  let release = summary_json(json!({
    "stage": "beta", "paused": false, "installs": 142, "installs_by_channel": { "stable": 96, "beta": 46 },
  }));
  let expected = json!({ "releases": [release], "installs_by_channel": { "stable": 414, "beta": 21 } });
  round_trip(&list, &expected);
}

#[test]
fn releases_summary_carries_the_ci_run_and_the_withdrawal() {
  let withdrawn = ReleaseSummary {
    ci_run: Some(CiRun {
      number: 44,
      url: CiRunUrl::parse(RUN_URL).expect("a link"),
    }),
    withdrawn: Some(Withdrawal {
      at: 1_792_200_000_000,
      by: "ops@example.com".to_owned(),
    }),
    ..summary(ReleaseStage::Withdrawn)
  };
  let expected = summary_json(json!({
    "stage": "withdrawn", "ci_run": { "number": 44, "url": RUN_URL },
    "withdrawn": { "at": 1_792_200_000_000_i64, "by": "ops@example.com" },
  }));
  round_trip(&withdrawn, &expected);
}

fn progress(state: UploadState, received: u32) -> FileProgress {
  FileProgress {
    name: file_name(NAME),
    platform: ReleasePlatform::Windows,
    arch: ReleaseArch::X86_64,
    size: 20_971_521,
    sha256: digest(),
    parts: 3,
    received,
    state,
  }
}

fn progress_json(state: Value, received: u32) -> Value {
  let file = json!({
    "name": NAME, "platform": "windows", "arch": "x86_64", "size": 20_971_521, "sha256": DIGEST, "parts": 3,
    "received": received,
  });
  with(file, state)
}

#[test]
fn releases_draft_carries_upload_progress_and_rejections() {
  let received = "9d2a40e7c5b1f3a8d6e4c2b0a9f8e7d6c5b4a3f2e1d0c9b8a7f6e5d4c3b271bf";
  let rejected = UploadState::Rejected {
    reason: Rejection::Sha256Mismatch,
    received_sha256: Some(FileDigest::parse(received).expect("a digest")),
  };
  let incomplete = UploadState::Rejected {
    reason: Rejection::Incomplete,
    received_sha256: None,
  };
  let draft = ReleaseSummary {
    published_at: None,
    installs: 0,
    uploads: vec![
      progress(UploadState::Receiving, 2),
      progress(UploadState::Verifying, 3),
      progress(UploadState::Verified, 3),
      progress(rejected, 3),
      progress(incomplete, 1),
    ],
    ..summary(ReleaseStage::Draft)
  };
  let uploads = json!([
    progress_json(json!({ "state": "receiving" }), 2),
    progress_json(json!({ "state": "verifying" }), 3),
    progress_json(json!({ "state": "verified" }), 3),
    progress_json(
      json!({ "state": "rejected", "reason": "sha256_mismatch", "received_sha256": received }),
      3
    ),
    progress_json(json!({ "state": "rejected", "reason": "incomplete" }), 1),
  ]);
  let expected = summary_json(json!({ "stage": "draft", "published_at": null, "installs": 0, "uploads": uploads }));
  round_trip(&draft, &expected);
  for (reason, rejection) in [
    ("signature_mismatch", Rejection::SignatureMismatch),
    ("unknown_key", Rejection::UnknownKey),
  ] {
    let state = json!({ "state": "rejected", "reason": reason, "received_sha256": DIGEST });
    let read: FileProgress = serde_json::from_value(progress_json(state, 3)).expect("read");
    let expected = UploadState::Rejected {
      reason: rejection,
      received_sha256: Some(digest()),
    };
    assert_eq!(read.state, expected);
  }
}

#[test]
fn releases_details_carry_files_adoption_and_activity() {
  let details = ReleaseDetails {
    summary: summary(ReleaseStage::Beta { paused: false }),
    notes: "Notes".to_owned(),
    files: vec![AdminFile {
      name: file_name(NAME),
      platform: ReleasePlatform::Windows,
      arch: ReleaseArch::X86_64,
      size: 48_213_504,
      sha256: digest(),
      key_id: key_id(),
      downloads: 980,
    }],
    adoption: vec![AdoptionDay {
      day: "2026-10-29".to_owned(),
      installs: 311,
      installs_by_channel: Some(ChannelInstalls { stable: 290, beta: 21 }),
    }],
    activity: vec![
      ReleaseActivity {
        at: 1_792_200_000_000,
        actor: Some("Ivan".to_owned()),
        action: ReleaseAction::Paused { channel: Channel::Beta },
      },
      ReleaseActivity {
        at: 1_792_100_000_000,
        actor: Some("Ivan".to_owned()),
        action: ReleaseAction::Promoted { percent: percent(10) },
      },
      ReleaseActivity {
        at: 1_792_000_000_000,
        actor: None,
        action: ReleaseAction::Created,
      },
    ],
  };
  let value = serde_json::to_value(&details).expect("written");
  assert_eq!(value["stage"], "beta");
  assert_eq!(value["files"][0]["downloads"], 980);
  assert_eq!(
    value["adoption"][0],
    json!({ "day": "2026-10-29", "installs": 311, "installs_by_channel": { "stable": 290, "beta": 21 } })
  );
  assert_eq!(
    value["activity"][0],
    json!({ "at": 1_792_200_000_000_i64, "actor": "Ivan", "action": "paused", "channel": "beta" })
  );
  assert_eq!(
    value["activity"][1],
    json!({ "at": 1_792_100_000_000_i64, "actor": "Ivan", "action": "promoted", "percent": 10 })
  );
  assert_eq!(
    value["activity"][2],
    json!({ "at": 1_792_000_000_000_i64, "actor": null, "action": "created" })
  );
  assert_eq!(serde_json::from_value::<ReleaseDetails>(value).expect("read"), details);
}

#[test]
fn releases_admin_requests_round_trip() {
  round_trip(&PercentChange { percent: percent(50) }, &json!({ "percent": 50 }));
  assert!(serde_json::from_value::<PercentChange>(json!({ "percent": 101 })).is_err());
  let changes = ReleaseChanges {
    notes: None,
    critical: Some(true),
  };
  round_trip(&changes, &json!({ "critical": true }));
  assert!(serde_json::from_value::<ReleaseChanges>(json!({ "channel": "beta" })).is_err());
}
