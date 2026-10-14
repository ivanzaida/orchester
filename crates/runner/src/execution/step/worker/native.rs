//! A native agent's side of a worker (`docs/architecture/native-agent.md` §2.2): its task's events are
//! taken in as a CLI's would be, a command that waits for a person goes through the approvals every
//! agent uses (auto mode's review and its hard rules included), the run's stops are handed to the loop,
//! which carries them out itself, and how the loop ended becomes the invocation's outcome. A result or a
//! question becomes the agent's last message, where the step's end reads it as for a CLI.

use orchester_core::{
  connection::stream::{AgentEvent, InvocationOutcome},
  native::{
    model::ModelError,
    session::{LoopEnd, LoopFailure, StopMode},
  },
  question::{Question, QuestionRequest, QuestionTurn, question_as_text},
  run::{StepOutcome, StepResult},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::StoreError;

use super::{
  super::native::{NativeHandle, NativeUpdate},
  Driver, FLUSH_INTERVAL, Finished, Stop, StopCause, Watch, Worker, review,
};

impl Worker {
  pub(super) async fn follow_native(mut self) -> Result<Finished, StoreError> {
    let mut watch = Watch::open(&self.root, &self.invocation)?;
    let mut flush = tokio::time::interval(FLUSH_INTERVAL);
    let mut listening = true;
    let mut deciding = true;
    loop {
      tokio::select! {
        update = next_update(&mut self.driver) => match update {
          Some(NativeUpdate::Activity(event)) => self.observe(&mut watch, event)?,
          Some(NativeUpdate::Approval(ask)) => self.request_native_approval(&mut watch, ask).await?,
          Some(NativeUpdate::Ended(end)) => return self.finish_native(watch, end),
          None => return self.finish_native(watch, Err("the native agent ended without saying how".to_owned())),
        },
        _ = flush.tick() => self.flush(&mut watch)?,
        decision = self.decisions.recv(), if deciding => match decision {
          Some(decision) => self.decide(&mut watch, decision).await?,
          None => deciding = false,
        },
        reviewed = review::next(&mut self.reviewing), if self.reviewing.is_some() => {
          self.apply_review(&mut watch, reviewed).await?;
        },
        stop = self.stops.recv(), if listening => match stop {
          Some(stop) => self.native_stop(&mut watch, stop),
          None => listening = false,
        },
      }
    }
  }

  /// Hands one of the run's stops to the loop, noting why it stops for the end.
  fn native_stop(&self, watch: &mut Watch, stop: Stop) {
    let Driver::Native(handle) = &self.driver else {
      return;
    };
    match stop {
      Stop::Now(cause) => {
        watch.stop.get_or_insert(cause);
        handle.stop(StopMode::Now);
      }
      Stop::AfterTurn(cause) => {
        watch.over.get_or_insert(cause);
        handle.stop(StopMode::AfterTurn);
      }
      Stop::AfterCommand(cause) => {
        watch.after_command.get_or_insert(cause);
        handle.stop(StopMode::AfterCommand);
      }
      Stop::AtApproval => {
        watch.pause_requested = true;
        handle.stop(StopMode::AtApproval);
      }
      Stop::Withdraw => {
        watch.after_command = None;
        watch.pause_requested = false;
        handle.withdraw();
      }
    }
  }

  /// How the invocation ended, from how its loop ended: a result or a question is its last message.
  fn finish_native(&self, mut watch: Watch, end: Result<LoopEnd, String>) -> Result<Finished, StoreError> {
    let (outcome, stopped) = match end {
      Ok(end) => self.loop_end(&mut watch, end)?,
      Err(message) => {
        self.observe(
          &mut watch,
          AgentEvent::TurnFailed {
            message: message.clone(),
          },
        )?;
        (InvocationOutcome::TurnFailed { message }, watch.stop)
      }
    };
    let approval_interrupted = watch.approval.is_some();
    if approval_interrupted {
      watch
        .db
        .interrupt_tool_approvals(watch.invocation.run, watch.invocation.id)?;
      watch.invocation.timing.resume(Timestamp::now());
      self.notifier.send(Event::ToolApprovalChanged {
        run: watch.invocation.run,
        task: self.task,
      });
    }
    self.flush(&mut watch)?;
    watch.invocation.ended_at = Some(Timestamp::now());
    watch.invocation.outcome = Some(outcome.clone());
    watch.db.save_invocation(&watch.invocation)?;
    Ok(Finished {
      outcome,
      stopped,
      final_message: watch.tracker.last_message().map(str::to_owned),
      approval_interrupted,
      closed_after_result: false,
    })
  }

  /// The outcome of a loop that ended as `end`, and why the run stopped it, if it did.
  fn loop_end(&self, watch: &mut Watch, end: LoopEnd) -> Result<(InvocationOutcome, Option<StopCause>), StoreError> {
    let result = match end {
      LoopEnd::Result(result) => result,
      LoopEnd::Question { call, question } => StepResult {
        outcome: StepOutcome::Question,
        summary: match question.structured() {
          Some(structured) => self.record_step_question(watch, call.as_str(), structured)?,
          None => question.question().to_owned(),
        },
        unverified: Vec::new(),
        completed: Vec::new(),
        decisions: Vec::new(),
        open_questions: Vec::new(),
        next_agent_should_know: Vec::new(),
      },
      LoopEnd::Stopped(mode) => {
        let cause = match mode {
          StopMode::Now => watch.stop,
          StopMode::AfterTurn => watch.over,
          StopMode::AfterCommand => watch.after_command,
          StopMode::AtApproval => Some(StopCause::Paused),
        };
        return Ok((InvocationOutcome::Terminated, cause.or(watch.stop)));
      }
      LoopEnd::NoResult => return Ok((InvocationOutcome::NoResult, None)),
      LoopEnd::Failed(failure) => return self.failed(watch, &failure).map(|outcome| (outcome, None)),
    };
    let text = serde_json::to_string(&result).expect("a step result serializes to JSON");
    self.observe(watch, AgentEvent::Message { text })?;
    Ok((InvocationOutcome::Completed, None))
  }

  /// A failed loop: a rejected key reads as not signed in, anything else as a failed turn.
  fn failed(&self, watch: &mut Watch, failure: &LoopFailure) -> Result<InvocationOutcome, StoreError> {
    let message = failure.to_string();
    if *failure == LoopFailure::Model(ModelError::KeyRejected) {
      self.observe(
        watch,
        AgentEvent::NotSignedIn {
          message: message.clone(),
        },
      )?;
      return Ok(InvocationOutcome::NotSignedIn { message });
    }
    self.observe(
      watch,
      AgentEvent::TurnFailed {
        message: message.clone(),
      },
    )?;
    Ok(InvocationOutcome::TurnFailed { message })
  }
}

impl Worker {
  /// Records the question with suggested answers that `call` ended the step with, which the person answers
  /// on its card; its text, with the options, is the agent's question message.
  fn record_step_question(&self, watch: &mut Watch, call: &str, question: Question) -> Result<String, StoreError> {
    let text = question_as_text(&question);
    let invocation = &watch.invocation;
    let request = QuestionRequest::new(
      format!("{}:{call}", invocation.id),
      (invocation.run, invocation.execution, invocation.id),
      self.agent.clone(),
      vec![question],
      (Timestamp::now(), QuestionTurn::StepEnd),
    )
    .expect("one question is asked");
    watch.db.ask_questions(&request)?;
    self.notifier.send(Event::QuestionChanged {
      run: invocation.run,
      task: self.task,
    });
    Ok(text)
  }
}

/// The next news of a native agent's task; a CLI's worker has none.
async fn next_update(driver: &mut Driver) -> Option<NativeUpdate> {
  match driver {
    Driver::Native(handle) => NativeHandle::next(handle).await,
    Driver::Process { .. } => std::future::pending().await,
  }
}
