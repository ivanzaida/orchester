//! The coordinator link's requests and answers (`docs/architecture/remote-runners.md`, "Rules of the boundary"):
//! each is a plain value that comes back the same from its JSON, so a remote coordinator can answer it. Run with
//! `cargo test -p orchester-runner --test coordinator_link`.

use std::{
  collections::{BTreeMap, BTreeSet},
  fmt::Debug,
};

use orchester_core::{
  attachment::{Attachment, AttachmentName, NewAttachment},
  blob::ContentHash,
  conversation::{Message, MessageBody, Recipient, Timing},
  id::{ConnectionId, ResourceId, RunId, TaskId, VersionId},
  name::Name,
  resource::{
    AgentProfile, AgentTargets, CommandAction, CommandProgram, InjectionEvent, Instruction, InstructionScope, ModelId,
    PromptText, ProviderSettings, Skill, SkillFile, SkillFilePath, Tool, VersionNumber,
  },
  run::{PinnedVersion, RunnerFailure},
  time::Timestamp,
};
use orchester_runner::execution::link::{
  LinkError,
  messages::{AttachedContent, AttachedRequest, MessagesRequest},
  resources::{
    Blob, BlobRequest, PinnedBody, PinnedKind, ResourceLink, ResourceNames, ResourceRequest, VersionRequest,
    pinned_version,
  },
};
use serde::{Serialize, de::DeserializeOwned};

/// `value`, written as JSON and read back.
fn round_trip<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
  let json = serde_json::to_string(value).expect("the value serializes");
  let back: T = serde_json::from_str(&json).expect("the JSON reads back");
  assert_eq!(&back, value, "{json}");
}

fn hash() -> ContentHash {
  ContentHash::new(&"ab".repeat(32)).expect("a valid hash")
}

fn profile() -> AgentProfile {
  AgentProfile::new(
    Name::new("Builder").expect("a name"),
    "Implements tasks.".to_owned(),
    PromptText::new("Work carefully.").expect("a prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("a model"),
    ProviderSettings::new(BTreeMap::from([("reasoning_effort".to_owned(), "low".to_owned())])).expect("settings"),
  )
}

fn instruction() -> Instruction {
  Instruction::new(
    PromptText::new("Keep files short.").expect("a prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("a valid instruction")
}

fn skill() -> Skill {
  let file = SkillFile {
    path: SkillFilePath::new("references/checks.md").expect("a path"),
    content: hash(),
    size: 10,
  };
  Skill::new(
    "Use when checks run.".to_owned(),
    "# Checks\n\nRun them.".to_owned(),
    vec![file],
    InstructionScope::Project,
  )
  .expect("a valid skill")
}

fn tool() -> Tool {
  Tool::command(
    "Runs the checks",
    CommandAction::new(CommandProgram::new("cargo").expect("a program"), Vec::new()),
  )
}

#[test]
fn coordinator_link_resource_requests_round_trip() {
  round_trip(&ResourceRequest {
    resource: ResourceId::generate(),
  });
  for kind in [
    PinnedKind::AgentProfile,
    PinnedKind::Instruction,
    PinnedKind::Skill,
    PinnedKind::Tool,
  ] {
    round_trip(&VersionRequest {
      kind,
      resource: ResourceId::generate(),
      number: VersionNumber::FIRST,
    });
  }
  round_trip(&BlobRequest { hash: hash() });
}

#[test]
fn coordinator_link_resource_answers_round_trip() {
  round_trip(&Some(ResourceNames {
    name: "Feature delivery".to_owned(),
    slug: "feature-delivery".to_owned(),
  }));
  round_trip(&None::<ResourceNames>);
  for body in [
    PinnedBody::AgentProfile(profile()),
    PinnedBody::Instruction(instruction()),
    PinnedBody::Skill(skill()),
    PinnedBody::Tool(tool()),
  ] {
    round_trip(&Some(body));
  }
  round_trip(&None::<PinnedBody>);
  round_trip(&Blob {
    content: b"cargo test".to_vec(),
  });
}

/// A 3 by 2 PNG header, which is taken as an image.
fn png() -> Vec<u8> {
  let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
  bytes.extend(3u32.to_be_bytes());
  bytes.extend(2u32.to_be_bytes());
  bytes.extend([8, 6, 0, 0, 0]);
  bytes
}

#[test]
fn coordinator_link_a_message_with_an_attachment_round_trips() {
  let (task, run) = (TaskId::generate(), RunId::generate());
  let body = MessageBody::new("Use this screenshot").expect("text");
  let message = Message::from_person(task, Some(run), Recipient::Orchestrator, body, Timestamp::now())
    .sent(Timing::Now)
    .expect("a person's message is sent now");
  round_trip(&MessagesRequest { task });
  round_trip(&vec![message.clone()]);
  round_trip(&AttachedRequest { message: message.id() });
  let file = NewAttachment::new(AttachmentName::new("shot.png").expect("a name"), png()).expect("an image");
  let attachment = Attachment::kept(&file, hash());
  let attached = vec![
    AttachedContent {
      attachment: attachment.clone(),
      content: Ok(png()),
    },
    AttachedContent {
      attachment,
      content: Err("the project's copy is gone".to_owned()),
    },
  ];
  round_trip(&attached);
  let shown = format!("{:?}", attached[0]);
  assert!(
    shown.contains("shot.png") && !shown.contains("137, 80, 78, 71"),
    "{shown}"
  );
}

#[test]
fn coordinator_link_errors_round_trip_with_their_kind_of_failure() {
  for failure in [
    RunnerFailure::StorageContention,
    RunnerFailure::StorageFailure,
    RunnerFailure::Other,
  ] {
    let error = LinkError {
      detail: "database is locked".to_owned(),
      failure,
    };
    round_trip(&error);
    assert_eq!(error.to_string(), "database is locked");
  }
}

/// A link that answers every version with a skill, whatever was asked.
struct SkillsOnly;

impl ResourceLink for SkillsOnly {
  fn resource(&self, _: &ResourceRequest) -> Result<Option<ResourceNames>, LinkError> {
    Ok(None)
  }

  fn version(&self, _: &VersionRequest) -> Result<Option<PinnedBody>, LinkError> {
    Ok(Some(PinnedBody::Skill(skill())))
  }

  fn blob(&self, _: &BlobRequest) -> Result<Blob, LinkError> {
    Ok(Blob { content: Vec::new() })
  }
}

#[test]
fn coordinator_link_pinned_version_takes_the_kind_it_asked_for_only() {
  let pin = PinnedVersion {
    resource: ResourceId::generate(),
    version: VersionId::generate(),
    number: VersionNumber::FIRST,
  };
  assert_eq!(
    pinned_version::<Skill>(&SkillsOnly, &pin).expect("a skill"),
    Some(skill())
  );
  let error = pinned_version::<Tool>(&SkillsOnly, &pin).expect_err("a skill is not a tool");
  assert_eq!(error.failure, RunnerFailure::Other);
}
