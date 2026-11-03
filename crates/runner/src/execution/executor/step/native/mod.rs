//! A native agent's invocation at work (`docs/architecture/native-agent.md` §2.2): a task of its own runs
//! core's loop ([`NativeLoop`]) against the model API, the file tools and the commands it may run, stores
//! each item of the conversation before what follows it (§6.2), and reports to the step's worker as a CLI's
//! output would: Activity events, a command that waits for a person, and how the loop ended. The worker
//! hands it the run's stops and the person's decisions ([`NativeHandle`]).

mod command;
mod drive;

use std::{path::PathBuf, sync::Arc};

use orchester_agent::{AnthropicClient, FileTools};
use orchester_core::{
  connection::stream::AgentEvent,
  id::{ConversationId, InvocationId},
  native::{
    command::ParsedCommand,
    item::ToolCallId,
    session::{LoopAction, LoopEnd, NativeLoop, StopMode},
  },
  run::CommandShell,
};
use tokio::{sync::mpsc, task::JoinHandle};

pub(super) use self::command::CommandHost;

/// Everything a native invocation's task works with.
pub(in super::super) struct NativeSession {
  /// The repository, whose project database the task stores the conversation in.
  pub(super) root: PathBuf,
  pub(super) conversation: ConversationId,
  pub(super) invocation: InvocationId,
  pub(super) client: AnthropicClient,
  pub(super) files: Arc<FileTools>,
  pub(super) commands: Arc<CommandHost>,
  pub(super) native: NativeLoop,
  /// What the loop asked for when it started or resumed.
  pub(super) first: Vec<LoopAction>,
}

/// A command the loop waits on a person for.
pub(in super::super) struct NativeAsk {
  pub(in super::super) call: ToolCallId,
  pub(in super::super) command: ParsedCommand,
  /// The shell a shell command goes to.
  pub(in super::super) shell: Option<CommandShell>,
}

/// What the task tells the worker.
pub(in super::super) enum NativeUpdate {
  Activity(AgentEvent),
  Approval(NativeAsk),
  /// The loop ended; `Err` says why the task could not carry it on (its conversation could not be
  /// stored, the worker stopped listening).
  Ended(Result<LoopEnd, String>),
}

/// What the worker tells the task.
enum Control {
  Stop(StopMode),
  /// A pause that has not happened yet is forgotten.
  Withdraw,
  Decided(ToolCallId, bool),
}

/// The worker's end of a native invocation's task. Dropping it ends the task, with the model request
/// or the command it waits on.
pub(in super::super) struct NativeHandle {
  updates: mpsc::UnboundedReceiver<NativeUpdate>,
  control: mpsc::UnboundedSender<Control>,
  task: JoinHandle<()>,
}

impl NativeHandle {
  /// Starts `session`'s task.
  pub(in super::super) fn spawn(session: NativeSession) -> Self {
    let (updates, received) = mpsc::unbounded_channel();
    let (control, controls) = mpsc::unbounded_channel();
    let task = tokio::spawn(drive::drive(session, updates, controls));
    Self {
      updates: received,
      control,
      task,
    }
  }

  /// The task's next news; `None` once it has ended and every update was read.
  pub(in super::super) async fn next(&mut self) -> Option<NativeUpdate> {
    self.updates.recv().await
  }

  pub(in super::super) fn stop(&self, mode: StopMode) {
    self.send(Control::Stop(mode));
  }

  pub(in super::super) fn withdraw(&self) {
    self.send(Control::Withdraw);
  }

  /// Hands the person's decision on `call`'s command to the loop.
  pub(in super::super) fn decide(&self, call: ToolCallId, approve: bool) -> Result<(), String> {
    self
      .control
      .send(Control::Decided(call, approve))
      .map_err(|_| "the native agent has ended".to_owned())
  }

  fn send(&self, control: Control) {
    // A task that has ended takes nothing more; how it ended is on its way to the worker.
    if self.control.send(control).is_err() {
      tracing::debug!("the native agent has already ended");
    }
  }
}

impl Drop for NativeHandle {
  fn drop(&mut self) {
    self.task.abort();
  }
}
