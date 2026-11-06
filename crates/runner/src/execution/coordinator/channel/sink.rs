//! The sink over the channel: each record crosses as JSON to the coordinator's side, which writes it with the sink it
//! holds (the local one), and what the write answers crosses back as JSON; a store error crosses as a value.

use std::sync::Arc;

use orchester_core::integration::RemovalStart;
use orchester_store::{
  ActivityRecord, AlwaysAllowAnswer, AlwaysAllowRecord, ApprovalRecord, InvocationRecord, InvocationStart,
  MemoryAnswer, MemoryRecord, MessageRecord, ModelRequirement, NoticeRecord, QuestionRecord, RemovalRecord, RuleAnswer,
  RuleRecord, RunLogRecord, RunStoreError, StepRecord, StoreError, WarmWorkspaceRecord, WorkspaceWritten,
};

use super::{Calls, Reply, Wire, kept_error, received, written};
use crate::execution::executor::sink::Sink;

type Backend = Box<dyn Sink>;

/// The executor's end of a sink whose other end, on the coordinator's side, is `Backend`.
pub(in crate::execution) struct ChannelSink {
  wire: Wire<Backend>,
}

impl ChannelSink {
  /// A sink whose records `sink` writes on the coordinator's side; each crossing is counted in `calls`.
  pub(in crate::execution) fn over(sink: Backend, calls: Arc<Calls>) -> Self {
    Self {
      wire: Wire::open(sink, serve, calls),
    }
  }
}

/// Writes the record `text` holds for `method` with `sink`'s method of that name.
fn serve(sink: &mut Backend, method: &str, text: &str) -> Reply {
  macro_rules! answer {
    ($($name:ident),* $(,)?) => {
      match method {
        $(stringify!($name) => written(sink.$name(received(text))),)*
        other => panic!("the sink has no record `{other}`"),
      }
    };
  }
  answer!(
    step,
    workspace,
    removal,
    notice,
    run_log,
    invocation_start,
    activity,
    invocation,
    message,
    question,
    approval,
    rule,
    always_allow,
    model_requirement,
    keep_memory,
  )
}

/// Methods that send their record over the channel and give the answer read back, or the store's error.
macro_rules! sent {
  ($($name:ident($record:ty) -> $answer:ty, $error:ty;)*) => {
    $(
      fn $name(&mut self, record: $record) -> Result<$answer, $error> {
        self.wire.ask(stringify!($name), &record).map_err(kept_error::<$error>)
      }
    )*
  };
}

impl Sink for ChannelSink {
  sent! {
    step(StepRecord) -> (), StoreError;
    workspace(WarmWorkspaceRecord) -> WorkspaceWritten, StoreError;
    removal(RemovalRecord) -> Option<RemovalStart>, StoreError;
    notice(NoticeRecord) -> bool, StoreError;
    run_log(RunLogRecord) -> (), RunStoreError;
    invocation_start(InvocationStart) -> u32, StoreError;
    activity(ActivityRecord) -> u64, StoreError;
    invocation(InvocationRecord) -> (), StoreError;
    message(MessageRecord) -> bool, StoreError;
    question(QuestionRecord) -> bool, StoreError;
    approval(ApprovalRecord) -> bool, StoreError;
    rule(RuleRecord) -> RuleAnswer, StoreError;
    always_allow(AlwaysAllowRecord) -> AlwaysAllowAnswer, StoreError;
    model_requirement(ModelRequirement) -> (), StoreError;
    keep_memory(MemoryRecord) -> MemoryAnswer, StoreError;
  }

  /// Another sink over a channel of its own, to the sink the coordinator's side opened beside its own.
  fn another(&self) -> Box<dyn Sink> {
    let opened = self.wire.on_coordinator_side(|sink: &mut Backend| sink.another());
    Box::new(Self::over(opened, self.wire.counted.clone()))
  }
}
