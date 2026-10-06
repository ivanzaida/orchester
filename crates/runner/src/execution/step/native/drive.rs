//! The task of a native invocation: what the loop asks for is done in order. Items are stored before
//! what follows them; events go to the worker; a model request, the tool calls of a turn and a person's
//! decision are waited on while the run's stops are taken in, and a stop that ends the loop drops what
//! was waited on (a request in flight, a running command and what it started).

use std::{collections::VecDeque, future::Future, sync::Arc};

use orchester_agent::{AnthropicClient, FileTools};
use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{ConversationId, InvocationId},
  native::{
    item::{Item, ToolCallId},
    model::{ModelClient, ModelRequest, StreamEvent},
    session::{ApprovalAsk, LoopAction, LoopEnd, LoopInput, NativeLoop, PlannedTool, ToolOutcome, ToolWork},
    store::NativeConversationStore,
  },
};
use orchester_store::ProjectDb;
use tokio::{sync::mpsc, task::JoinSet};

use super::{CommandHost, Control, NativeAsk, NativeSession, NativeUpdate};

pub(super) async fn drive(
  session: NativeSession,
  updates: mpsc::UnboundedSender<NativeUpdate>,
  controls: mpsc::UnboundedReceiver<Control>,
) {
  let NativeSession {
    root,
    conversation,
    invocation,
    client,
    files,
    commands,
    native,
    first,
  } = session;
  let end = match ProjectDb::open(&root) {
    Ok(db) => {
      let mut steer = Steer {
        native,
        controls,
        out: Out {
          db,
          conversation,
          invocation,
          files: Arc::clone(&files),
          updates: updates.clone(),
          pending: VecDeque::new(),
        },
      };
      let work = Work {
        client,
        files,
        commands,
      };
      run(&mut steer, &work, first).await
    }
    Err(error) => Err(format!("cannot open the project database: {error}")),
  };
  // The worker that would read the end has gone; the runner is ending the agent anyway.
  if updates.send(NativeUpdate::Ended(end)).is_err() {
    tracing::debug!("nobody waits for the native agent's end");
  }
}

/// What the loop is told and where what it says goes.
struct Steer {
  native: NativeLoop,
  controls: mpsc::UnboundedReceiver<Control>,
  out: Out,
}

/// Where the loop's stored items, events and next steps go.
struct Out {
  db: ProjectDb,
  conversation: ConversationId,
  invocation: InvocationId,
  files: Arc<FileTools>,
  updates: mpsc::UnboundedSender<NativeUpdate>,
  /// What the loop asked for that waits on something: a model request, tool calls, a decision, its end.
  pending: VecDeque<LoopAction>,
}

/// What does the loop's work.
struct Work {
  client: AnthropicClient,
  files: Arc<FileTools>,
  commands: Arc<CommandHost>,
}

/// Something waited on, or the loop ended by a stop first.
enum Waited<T> {
  Done(T),
  Ended,
}

async fn run(steer: &mut Steer, work: &Work, first: Vec<LoopAction>) -> Result<LoopEnd, String> {
  steer.out.apply(first)?;
  loop {
    let Some(action) = steer.out.pending.pop_front() else {
      return Err("the native loop asked for nothing more without ending".to_owned());
    };
    match action {
      LoopAction::End(end) => return Ok(end),
      LoopAction::CallModel(request) => call_model(steer, work, &request).await?,
      LoopAction::Run(batch) => run_tools(steer, work, batch).await?,
      LoopAction::Ask(ask) => decide(steer, ask).await?,
      LoopAction::Persist(_) | LoopAction::Emit(_) => steer.out.apply(vec![action])?,
    }
  }
}

async fn call_model(steer: &mut Steer, work: &Work, request: &ModelRequest) -> Result<(), String> {
  let updates = steer.out.updates.clone();
  let mut on_event = move |event: StreamEvent| {
    if let Some(event) = stream_activity(event) {
      // The worker has gone only when the agent is ending.
      let _ = updates.send(NativeUpdate::Activity(event));
    }
  };
  let response = work.client.respond(request, &mut on_event);
  match steer.until(response).await? {
    Waited::Done(Ok(response)) => steer.feed(LoopInput::Responded(response)),
    Waited::Done(Err(error)) => steer.feed(LoopInput::ModelFailed(error)),
    Waited::Ended => Ok(()),
  }
}

/// What a streamed response's progress tells Activity: a retry after a failed attempt, reasoning the
/// endpoint refused, and the tokens of a response that was dropped, which budgets count.
fn stream_activity(event: StreamEvent) -> Option<AgentEvent> {
  let problem = match event {
    StreamEvent::Retrying { attempt, wait, error } => RunnerProblem::ModelRetrying {
      attempt,
      wait_seconds: wait.as_secs(),
      error: error.to_string(),
    },
    StreamEvent::ThinkingStripped => RunnerProblem::ThinkingStripped,
    StreamEvent::Spent(usage) => return Some(AgentEvent::TurnCompleted { usage }),
    StreamEvent::Text(_) | StreamEvent::Reasoning(_) | StreamEvent::ToolCallStarted { .. } => return None,
  };
  Some(AgentEvent::RunnerProblem { problem })
}

/// Runs a turn's calls (several only when they all read) and tells the loop of each as it ends.
async fn run_tools(steer: &mut Steer, work: &Work, batch: Vec<PlannedTool>) -> Result<(), String> {
  let mut running = JoinSet::new();
  for planned in batch {
    running.spawn(tool(Arc::clone(&work.files), Arc::clone(&work.commands), planned));
  }
  while !running.is_empty() {
    match steer.until(running.join_next()).await? {
      Waited::Done(Some(Ok((call, outcome)))) => steer.feed(LoopInput::ToolDone(call, outcome))?,
      Waited::Done(Some(Err(error))) => return Err(format!("a tool call failed to run: {error}")),
      Waited::Done(None) | Waited::Ended => break,
    }
  }
  Ok(())
}

/// One call's work: a file tool off the async threads, a command under the supervisor.
async fn tool(files: Arc<FileTools>, commands: Arc<CommandHost>, planned: PlannedTool) -> (ToolCallId, ToolOutcome) {
  let PlannedTool { call, work } = planned;
  let outcome = match work {
    ToolWork::Command(command) => commands.run(&command).await,
    file_work => match tokio::task::spawn_blocking(move || file_tool(&files, &file_work)).await {
      Ok(outcome) => outcome,
      Err(error) => CommandHost::failed(format!("the file tool stopped: {error}")),
    },
  };
  (call, outcome)
}

fn file_tool(files: &FileTools, work: &ToolWork) -> ToolOutcome {
  match work {
    ToolWork::ReadFile(input) => files.read_file(input),
    ToolWork::ListFiles(input) => files.list_files(input),
    ToolWork::Search(input) => files.search(input),
    ToolWork::EditFile(input) => files.edit_file(input),
    ToolWork::WriteFile(input) => files.write_file(input),
    ToolWork::Command(_) => CommandHost::failed("a command is not a file tool".to_owned()),
  }
}

/// Asks the worker to have a person decide `ask`'s command, and tells the loop the decision.
async fn decide(steer: &mut Steer, ask: ApprovalAsk) -> Result<(), String> {
  let shell = steer.native.shell();
  let asked = NativeAsk {
    call: ask.call,
    command: ask.command,
    shell,
  };
  steer
    .out
    .updates
    .send(NativeUpdate::Approval(asked))
    .map_err(|_| "the worker no longer follows the agent".to_owned())?;
  loop {
    match steer.controls.recv().await {
      Some(Control::Decided(call, approve)) => return steer.feed(LoopInput::Decided(call, approve)),
      Some(control) => {
        if steer.control(control)? {
          return Ok(());
        }
      }
      None => return Err("the worker no longer follows the agent".to_owned()),
    }
  }
}

impl Steer {
  /// Waits for `work` while taking the run's stops; [`Waited::Ended`] when one ended the loop first.
  async fn until<T>(&mut self, work: impl Future<Output = T>) -> Result<Waited<T>, String> {
    tokio::pin!(work);
    loop {
      tokio::select! {
        done = &mut work => return Ok(Waited::Done(done)),
        control = self.controls.recv() => match control {
          Some(control) => {
            if self.control(control)? {
              return Ok(Waited::Ended);
            }
          }
          None => return Err("the worker no longer follows the agent".to_owned()),
        },
      }
    }
  }

  /// Takes in a control of the run; `true` when it ended the loop.
  fn control(&mut self, control: Control) -> Result<bool, String> {
    let input = match control {
      Control::Stop(mode) => LoopInput::Stop(mode),
      Control::Withdraw => LoopInput::Withdraw,
      Control::Decided(call, _) => {
        tracing::debug!(%call, "a decision arrived for a call that does not wait for one");
        return Ok(false);
      }
    };
    self.feed(input)?;
    Ok(self.native.has_ended())
  }

  fn feed(&mut self, input: LoopInput) -> Result<(), String> {
    let actions = self.native.handle(input).map_err(|error| error.to_string())?;
    self.out.apply(actions)
  }
}

impl Out {
  /// Stores and reports what `actions` say at once; the rest waits its turn.
  fn apply(&mut self, actions: Vec<LoopAction>) -> Result<(), String> {
    for action in actions {
      match action {
        LoopAction::Persist(items) => self.persist(&items)?,
        LoopAction::Emit(events) => {
          for event in events {
            self
              .updates
              .send(NativeUpdate::Activity(event))
              .map_err(|_| "the worker no longer follows the agent".to_owned())?;
          }
        }
        waiting => self.pending.push_back(waiting),
      }
    }
    Ok(())
  }

  fn persist(&mut self, items: &[Item]) -> Result<(), String> {
    self
      .db
      .append_native_items(self.conversation, self.invocation, items)
      .map_err(|error| format!("cannot store the conversation: {error}"))?;
    self.files.stored(items);
    Ok(())
  }
}
