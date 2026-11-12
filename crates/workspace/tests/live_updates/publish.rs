//! What Share's tests send the team's configuration through: a person's server as Share uploads to it ([`Publisher`]),
//! and the bodies they upload.

use std::collections::BTreeSet;

use orchester_api::{
  FileStored, ResourceCreate, ResourceCreated, ResourcesImport, SharedResource, SharedVersion, VersionPublish, Versions,
};
use orchester_core::{
  blob::ContentHash,
  id::ResourceId,
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentTargets, InjectionEvent, Instruction, InstructionScope, Pipeline, PromptText, RepositoryAccess, Step, StepKey,
    StepKind, VersionNumber,
  },
};
use orchester_workspace::sharing::{SharingError, sync::ConfigServer};
use serde_json::Value;

use super::{Member, TeamServer};

/// A person's server as Share uploads to it.
pub(super) struct Publisher {
  member: Member,
}

impl Publisher {
  /// `token`'s member's server.
  pub(super) fn online(server: &TeamServer, token: &str) -> Self {
    Self {
      member: Member::of(server, token),
    }
  }
}

impl ConfigServer for Publisher {
  async fn versions(&self, resource: ResourceId, after: Option<VersionNumber>) -> Result<Versions, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .versions(&member.token, member.project, resource, after)
        .await?,
    )
  }

  async fn create_resource(&self, create: ResourceCreate) -> Result<ResourceCreated, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .create_resource(&member.token, member.project, &create)
        .await?,
    )
  }

  async fn publish_version(
    &self,
    resource: ResourceId,
    publish: VersionPublish,
  ) -> Result<SharedVersion, SharingError> {
    let member = &self.member;
    let published = member
      .client
      .publish_version(&member.token, member.project, resource, &publish);
    Ok(published.await?)
  }

  async fn upload_file(&self, hash: ContentHash, content: Vec<u8>) -> Result<FileStored, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .upload_file(&member.token, member.project, &hash, content)
        .await?,
    )
  }

  async fn import_resources(&self, import: ResourcesImport) -> Result<Vec<SharedResource>, SharingError> {
    let member = &self.member;
    Ok(
      member
        .client
        .import_resources(&member.token, member.project, &import)
        .await?,
    )
  }
}

pub(super) fn rule(text: &str) -> Instruction {
  Instruction::new(
    PromptText::new(text).expect("a prompt"),
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Project,
  )
  .expect("a valid instruction")
}

pub(super) fn json(text: &str) -> Value {
  serde_json::to_value(rule(text)).expect("an instruction serializes")
}

pub(super) fn number(value: u32) -> VersionNumber {
  VersionNumber::new(value.try_into().expect("above zero"))
}

/// One step, Implement, run by `profile`.
pub(super) fn one_step(profile: ResourceId) -> Pipeline {
  let step = Step {
    key: StepKey::new("implement").expect("key"),
    name: Name::new("Implement").expect("name"),
    kind: StepKind::Agent {
      profile: Some(profile),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("Say hello.").expect("prompt"),
    depends_on: BTreeSet::new(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  Pipeline::new(vec![step]).expect("pipeline")
}
