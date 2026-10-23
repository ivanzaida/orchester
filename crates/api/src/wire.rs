//! Wire names preserve core's existing local-storage encoding.
use orchester_core::{
  module::ModuleState,
  run::TaskPhase,
  task::{
    TaskStatus,
    organization::{ChangeActor, DependencyGate, Priority, WorkflowStatus},
  },
};
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize)]
#[serde(remote = "ModuleState", rename_all = "snake_case")]
pub enum ModuleStateWire {
  Open,
  Closed,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "WorkflowStatus", rename_all = "snake_case")]
pub enum Workflow {
  Backlog,
  Todo,
  InProgress,
  InReview,
  Done,
  Canceled,
}
pub mod optional_workflow {
  use super::*;
  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "Workflow")] WorkflowStatus);
  pub fn serialize<S: serde::Serializer>(value: &Option<WorkflowStatus>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(Value).serialize(serializer)
  }
  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<WorkflowStatus>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.map(|value| value.0))
  }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "Priority", rename_all = "snake_case")]
pub enum PriorityWire {
  Urgent,
  High,
  Medium,
  Low,
  None,
}
pub mod optional_priority_wire {
  use super::*;
  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "PriorityWire")] Priority);
  pub fn serialize<S: serde::Serializer>(value: &Option<Priority>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(Value).serialize(serializer)
  }
  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Priority>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.map(|value| value.0))
  }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "DependencyGate", rename_all = "snake_case")]
pub enum Gate {
  Review,
  Merge,
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "ChangeActor", rename_all = "snake_case")]
pub enum Actor {
  Person,
  Orchester,
}
pub mod optional_actor {
  use super::*;
  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "Actor")] ChangeActor);
  pub fn serialize<S: serde::Serializer>(value: &Option<ChangeActor>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(Value).serialize(serializer)
  }
  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<ChangeActor>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.map(|value| value.0))
  }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "TaskStatus", rename_all = "snake_case")]
pub enum Status {
  Draft,
  Queued,
}
pub mod optional_status {
  use super::*;
  #[derive(Serialize, Deserialize)]
  struct Value(#[serde(with = "Status")] TaskStatus);
  pub fn serialize<S: serde::Serializer>(value: &Option<TaskStatus>, serializer: S) -> Result<S::Ok, S::Error> {
    value.map(Value).serialize(serializer)
  }
  pub fn deserialize<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<TaskStatus>, D::Error> {
    Ok(Option::<Value>::deserialize(deserializer)?.map(|value| value.0))
  }
}
#[derive(Serialize, Deserialize)]
#[serde(remote = "TaskPhase", rename_all = "snake_case")]
pub enum Phase {
  Draft,
  Queued,
  Running,
  Waiting,
  Stuck,
  Paused,
  StoppedAtLimit,
  Interrupted,
  Succeeded,
  Merged,
  Failed,
  Cancelled,
}
