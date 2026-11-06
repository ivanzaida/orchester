//! A module as an S10 read gives it: every public field of a shared module ([`SharedModule`]), then its branch, its
//! assignee, its work, its effective claim, the caller's capabilities on it and its open delivery to base. Its tasks
//! are read with the task list's module filter ([`super::TaskFilters::modules`]). Every field is sent; one without a
//! value is `null`, and an unknown field is refused.

use orchester_core::{
  id::ModuleId,
  module::{Horizon, ModuleState},
  sharing::TaskRevision,
};
use serde::{Deserialize, Serialize};

use super::{Capabilities, ClaimRead, PendingDeliveryRead, WorkRead, page::nullable};
use crate::{Attribution, ModuleFields, SharedModule, UserRef, wire};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "ModuleReadWire", into = "ModuleReadWire")]
pub struct ModuleRead {
  pub module: SharedModule,
  /// The module's branch, which its tasks' work builds on.
  pub branch: String,
  pub assignee: Option<UserRef>,
  pub work: Option<WorkRead>,
  pub effective_claim: Option<ClaimRead>,
  pub capabilities: Capabilities,
  /// The delivery of the module branch to base that is requested and not yet delivered or failed, `None` when none is.
  pub pending_delivery: Option<PendingDeliveryRead>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ModuleReadWire {
  id: ModuleId,
  name: String,
  description: String,
  #[serde(deserialize_with = "nullable")]
  start: Option<String>,
  #[serde(deserialize_with = "nullable")]
  target: Option<String>,
  #[serde(with = "wire::ModuleStateWire")]
  state: ModuleState,
  horizon: Horizon,
  place: u32,
  revision: TaskRevision,
  created: Attribution,
  updated: Attribution,
  branch: String,
  #[serde(deserialize_with = "nullable")]
  assignee: Option<UserRef>,
  #[serde(deserialize_with = "nullable")]
  work: Option<WorkRead>,
  #[serde(deserialize_with = "nullable")]
  effective_claim: Option<ClaimRead>,
  capabilities: Capabilities,
  #[serde(deserialize_with = "nullable")]
  pending_delivery: Option<PendingDeliveryRead>,
}

impl From<ModuleReadWire> for ModuleRead {
  fn from(wire: ModuleReadWire) -> Self {
    let fields = ModuleFields {
      name: wire.name,
      description: wire.description,
      start: wire.start,
      target: wire.target,
      state: wire.state,
      horizon: wire.horizon,
      place: wire.place,
    };
    Self {
      module: SharedModule {
        id: wire.id,
        fields,
        revision: wire.revision,
        created: wire.created,
        updated: wire.updated,
      },
      branch: wire.branch,
      assignee: wire.assignee,
      work: wire.work,
      effective_claim: wire.effective_claim,
      capabilities: wire.capabilities,
      pending_delivery: wire.pending_delivery,
    }
  }
}

impl From<ModuleRead> for ModuleReadWire {
  fn from(read: ModuleRead) -> Self {
    let SharedModule {
      id,
      fields,
      revision,
      created,
      updated,
    } = read.module;
    Self {
      id,
      name: fields.name,
      description: fields.description,
      start: fields.start,
      target: fields.target,
      state: fields.state,
      horizon: fields.horizon,
      place: fields.place,
      revision,
      created,
      updated,
      branch: read.branch,
      assignee: read.assignee,
      work: read.work,
      effective_claim: read.effective_claim,
      capabilities: read.capabilities,
      pending_delivery: read.pending_delivery,
    }
  }
}
