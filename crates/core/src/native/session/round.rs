//! One model turn's tool calls: planned in the order §4.10 gives, then run one step at a time.

use super::{
  LoopAction, LoopEnd, LoopError, LoopFailure, NativeLoop, Phase, PlannedTool, StopMode, ToolOutcome, ToolWork,
  events::{self, Shown},
};
use crate::{
  connection::stream::DeclinedBy,
  native::{
    command::{CommandForm, CommandVerdict},
    item::{Item, ToolCallId, UserOrigin},
    model::{ModelResponse, StopReason},
    session::ApprovalAsk,
    tools::{RunCommand, ToolInput, ToolInputError, parse_call},
  },
  run::{CommandShell, StepResult},
};

/// What the model is told when it ends a turn without a tool call (§4.7).
pub const REMINDER: &str = "Call finish_step with your result.";
/// The result of a command a person denied (§5.2).
pub const DENIED: &str = "Denied by a person. Do not run this command again; continue without it or explain in \
                          finish_step why you cannot.";
/// The result of the `finish_step` call whose result the loop ended with. It is stored, so that a
/// conversation resumed after the step was reopened does not tell the model its result was not taken.
pub const RECORDED: &str = "Your result was recorded.";

/// A planned step of the current turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Queued {
  /// The turn's reading calls, run together.
  Reads(Vec<PlannedTool>),
  Write(PlannedTool),
  /// Classified only when its turn comes, since the calls before it can change the files it names.
  Command(ToolCallId, RunCommand),
  Finish(ToolCallId, StepResult),
  End(LoopEnd),
}

impl NativeLoop {
  pub(super) fn on_response(&mut self, response: ModelResponse, actions: &mut Vec<LoopAction>) {
    self.context.responded(response.usage);
    let calls: Vec<(ToolCallId, String, serde_json::Value)> = response
      .items
      .iter()
      .filter_map(|item| match item {
        Item::ToolCall { id, name, input } => Some((id.clone(), name.clone(), input.clone())),
        _ => None,
      })
      .collect();
    let emitted = events::response(&response);
    self.append(response.items, actions);
    actions.push(LoopAction::Emit(emitted));
    if self.stop == Some(StopMode::AfterTurn) {
      self.end(LoopEnd::Stopped(StopMode::AfterTurn), actions);
      return;
    }
    match response.stop {
      StopReason::Refusal { category } => return self.end(LoopEnd::Failed(LoopFailure::Refused { category }), actions),
      StopReason::Filtered => return self.end(LoopEnd::Failed(LoopFailure::Filtered), actions),
      StopReason::ContextExceeded if calls.is_empty() => {
        self.context.force();
        return self.next_request(actions);
      }
      StopReason::ContextExceeded => self.context.force(),
      StopReason::EndTurn | StopReason::ToolUse | StopReason::MaxOutput => {}
    }
    if calls.is_empty() {
      return self.on_missing_finish(actions);
    }
    self.reminded = false;
    self.plan(calls, actions);
    self.advance(actions);
  }

  /// Queues the calls of a turn: the reading ones first, the rest in order, nothing after a call that
  /// ends the loop. Calls that cannot run are answered at once.
  fn plan(&mut self, calls: Vec<(ToolCallId, String, serde_json::Value)>, actions: &mut Vec<LoopAction>) {
    let mut reads = Vec::new();
    let mut rest = Vec::new();
    let mut invalid = Vec::new();
    for (call, name, input) in calls {
      let input = match self.parse(&name, &input) {
        Ok(input) => input,
        Err(error) => {
          invalid.push((call, name, error.to_string()));
          continue;
        }
      };
      let planned = |work| PlannedTool {
        call: call.clone(),
        work,
      };
      match input {
        ToolInput::ReadFile(read) => reads.push(planned(ToolWork::ReadFile(read))),
        ToolInput::ListFiles(list) => reads.push(planned(ToolWork::ListFiles(list))),
        ToolInput::Search(search) => reads.push(planned(ToolWork::Search(search))),
        ToolInput::EditFile(edit) => rest.push(Queued::Write(planned(ToolWork::EditFile(edit)))),
        ToolInput::WriteFile(write) => rest.push(Queued::Write(planned(ToolWork::WriteFile(write)))),
        ToolInput::RunCommand(command) => rest.push(Queued::Command(call, command)),
        ToolInput::FinishStep(result) => {
          rest.push(Queued::Finish(call, result));
          break;
        }
        ToolInput::AskQuestion(question) => {
          rest.push(Queued::End(LoopEnd::Question { call, question }));
          break;
        }
      }
    }
    if !invalid.is_empty() {
      let emitted = invalid
        .iter()
        .flat_map(|(call, name, error)| events::invalid(call, name, error))
        .collect();
      let results = invalid
        .into_iter()
        .map(|(call, _, error)| Item::tool_result(call, error, true))
        .collect();
      self.append(results, actions);
      actions.push(LoopAction::Emit(emitted));
    }
    if !reads.is_empty() {
      self.queue.push_back(Queued::Reads(reads));
    }
    self.queue.extend(rest);
  }

  fn parse(&self, name: &str, input: &serde_json::Value) -> Result<ToolInput, ToolInputError> {
    let input = parse_call(name, input)?;
    if self.settings.tools.offers(input.name()) {
      Ok(input)
    } else {
      Err(ToolInputError::NotOffered(input.name()))
    }
  }

  /// Starts the next queued step, or the next model request when the turn's calls are done.
  pub(super) fn advance(&mut self, actions: &mut Vec<LoopAction>) {
    loop {
      if let Some(mode @ (StopMode::AfterTurn | StopMode::AfterCommand)) = self.stop {
        return self.end(LoopEnd::Stopped(mode), actions);
      }
      let Some(next) = self.queue.pop_front() else {
        return self.next_request(actions);
      };
      match next {
        Queued::Reads(batch) => return self.run(batch, actions),
        Queued::Write(planned) => return self.run(vec![planned], actions),
        Queued::Finish(call, result) => {
          self.append(vec![Item::tool_result(call, RECORDED, false)], actions);
          return self.end(LoopEnd::Result(result), actions);
        }
        Queued::End(end) => return self.end(end, actions),
        Queued::Command(call, input) => match self.commands.classify(&input, &self.scope) {
          CommandVerdict::Refuse(refusal) => {
            let command = (input.command(), events::command_shell(input.shell(), &self.settings));
            self.decline(call, command, (DeclinedBy::Orchester, refusal.to_string()), actions);
          }
          CommandVerdict::Run(command) => {
            let work = ToolWork::Command(command);
            return self.run(vec![PlannedTool { call, work }], actions);
          }
          CommandVerdict::Ask(_) if self.stop == Some(StopMode::AtApproval) => {
            return self.end(LoopEnd::Stopped(StopMode::AtApproval), actions);
          }
          CommandVerdict::Ask(command) => {
            self.phase = Phase::AwaitingDecision(call.clone(), command.clone());
            actions.push(LoopAction::Ask(ApprovalAsk { call, command }));
            return;
          }
        },
      }
    }
  }

  fn run(&mut self, batch: Vec<PlannedTool>, actions: &mut Vec<LoopAction>) {
    let running: Vec<Shown> = batch
      .iter()
      .map(|planned| Shown::of(&planned.call, &planned.work, &self.settings))
      .collect();
    actions.push(LoopAction::Emit(running.iter().map(events::started).collect()));
    actions.push(LoopAction::Run(batch));
    self.phase = Phase::Running(running);
  }

  pub(super) fn on_tool_done(
    &mut self,
    call: ToolCallId,
    outcome: ToolOutcome,
    actions: &mut Vec<LoopAction>,
  ) -> Result<(), LoopError> {
    let Phase::Running(running) = &mut self.phase else {
      return Err(self.unexpected("a tool outcome"));
    };
    let Some(index) = running.iter().position(|shown| shown.call == call) else {
      return Err(LoopError::UnknownCall(call));
    };
    let shown = running.remove(index);
    let done = running.is_empty();
    self
      .changed
      .extend(outcome.changes.iter().map(|change| change.path.clone()));
    let emitted = events::finished(&shown, &outcome);
    self.append(
      vec![Item::tool_result(shown.call, outcome.content, outcome.is_error)],
      actions,
    );
    actions.push(LoopAction::Emit(emitted));
    if done {
      self.advance(actions);
    }
    Ok(())
  }

  pub(super) fn on_decided(
    &mut self,
    call: ToolCallId,
    approved: bool,
    actions: &mut Vec<LoopAction>,
  ) -> Result<(), LoopError> {
    let command = match std::mem::replace(&mut self.phase, Phase::Ended) {
      Phase::AwaitingDecision(waiting, command) if waiting == call => command,
      Phase::AwaitingDecision(waiting, command) => {
        self.phase = Phase::AwaitingDecision(waiting, command);
        return Err(LoopError::UnknownCall(call));
      }
      other => {
        self.phase = other;
        return Err(self.unexpected("a decision"));
      }
    };
    if approved {
      let work = ToolWork::Command(command);
      self.run(vec![PlannedTool { call, work }], actions);
    } else {
      let shell = events::command_shell(matches!(command.form(), CommandForm::Shell), &self.settings);
      let declined = (DeclinedBy::Person, DENIED.to_owned());
      self.decline(call, (command.text(), shell), declined, actions);
      self.advance(actions);
    }
    Ok(())
  }

  /// Answers a command that does not run with `message`, and records that `by` declined it.
  fn decline(
    &mut self,
    call: ToolCallId,
    command: (&str, Option<CommandShell>),
    (by, message): (DeclinedBy, String),
    actions: &mut Vec<LoopAction>,
  ) {
    let declined = events::declined(&call, command, by, &message);
    self.append(vec![Item::tool_result(call, message, true)], actions);
    actions.push(LoopAction::Emit(declined));
  }

  /// A turn ended without a tool call: remind the model, then, when the next turn has no tool call
  /// either, take a result from its last message if it wrote one there.
  fn on_missing_finish(&mut self, actions: &mut Vec<LoopAction>) {
    if !self.reminded {
      self.reminded = true;
      self.append(vec![Item::user(REMINDER, UserOrigin::Reminder)], actions);
      return self.next_request(actions);
    }
    let last_text = self.items.iter().rev().find_map(|item| match item {
      Item::Assistant { text } => Some(text.as_str()),
      _ => None,
    });
    let end = last_text
      .and_then(StepResult::at_end)
      .map_or(LoopEnd::NoResult, LoopEnd::Result);
    self.end(end, actions);
  }
}
