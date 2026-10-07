//! What a worker does with each event of its agent, whatever drives it: the event is redacted,
//! followed for the invocation's session, usage and first edit, recorded in batches, and a turn's
//! usage is handed to the run. Plan limits are not part of the transcript: they go to the desktop and to
//! the plan limits history.

use orchester_core::{
  connection::{
    AgentProvider,
    plan_limits::{LimitReading, PlanLimits},
    stream::AgentEvent,
  },
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::StoreError;

use super::{Driver, FLUSH_BATCH, Transport, Watch, Worker, WorkerEvent};

impl Worker {
  /// Takes in one event: a turn's usage goes to the run, and a new turn after a used-up budget
  /// ends the agent.
  pub(in super::super) fn observe(&self, watch: &mut Watch, mut event: AgentEvent) -> Result<(), StoreError> {
    if let AgentEvent::PlanLimits { limits } = event {
      self.publish_limits(watch, limits);
      return Ok(());
    }
    if self.app_server().is_some() || !self.secrets.is_empty() {
      event.redact(&|text| self.redact(text));
    }
    let now = Timestamp::now();
    watch.tracker.observe(&event);
    watch.edits_changed |= watch.edits.observe(&event, now);
    let usage = match &event {
      AgentEvent::TurnCompleted { usage } => Some(*usage),
      _ => None,
    };
    if event == AgentEvent::TurnStarted && watch.stop.is_none() {
      watch.stop = watch.over;
    }
    match &event {
      AgentEvent::CommandStarted { item, .. } => {
        watch.commands.insert(item.clone());
      }
      AgentEvent::CommandFinished { item, .. } => {
        watch.commands.remove(item);
        watch.approved_calls.remove(item);
      }
      _ => {}
    }
    watch.pending.push((now, event));
    if watch.pending.len() >= FLUSH_BATCH || usage.is_some() {
      self.flush(watch)?;
    }
    if let Some(usage) = usage {
      let usage = WorkerEvent::Usage {
        execution: watch.invocation.execution,
        tokens: usage.counted_tokens(),
      };
      if self.events.send(usage).is_err() {
        tracing::debug!("the run no longer counts its agent's usage");
      }
    }
    Ok(())
  }

  /// Stores the account's limits the CLI reported in the plan limits history and hands them to the
  /// desktop, as of now.
  fn publish_limits(&self, watch: &mut Watch, limits: PlanLimits) {
    let provider = match &self.driver {
      Driver::Process {
        transport: Transport::AppServer(_),
        ..
      } => AgentProvider::CodexCli,
      Driver::Process {
        transport: Transport::Claude(_),
        ..
      } => AgentProvider::ClaudeCodeCli,
      // A model API reports no account limits.
      Driver::Native(_) => return,
    };
    let reading = LimitReading {
      provider,
      at: Timestamp::now(),
      limits,
    };
    // The history only lacks this report, which its chart shows as a gap; the agent works on.
    if let Err(error) = watch.db.save_limit_snapshot(watch.invocation.run, &reading) {
      tracing::warn!(%error, "cannot store the plan limits the agent's CLI reported");
    }
    self.notifier.send(Event::PlanLimits(reading));
  }

  pub(super) fn flush(&self, watch: &mut Watch) -> Result<(), StoreError> {
    if watch.pending.is_empty() {
      return Ok(());
    }
    // A resumed session may not report its id again; the invocation keeps the one it resumed.
    let session = watch
      .tracker
      .session()
      .map(str::to_owned)
      .or_else(|| watch.invocation.session.clone());
    if watch.invocation.session != session || watch.invocation.usage != watch.tracker.usage() {
      watch.invocation.session = session;
      watch.invocation.usage = watch.tracker.usage();
      watch.db.save_invocation(&watch.invocation)?;
    }
    if watch.edits_changed {
      watch
        .db
        .save_edit_progress(watch.invocation.id, watch.edits.progress())?;
      watch.edits_changed = false;
    }
    let events = std::mem::take(&mut watch.pending);
    let sequence = watch.db.append_activity(&watch.invocation, &events)?;
    self.notifier.send(Event::AgentActivity {
      run: watch.invocation.run,
      sequence,
    });
    Ok(())
  }
}
