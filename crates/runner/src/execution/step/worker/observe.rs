//! What a worker does with each event of its agent, whatever drives it: the event is redacted,
//! followed for the invocation's session, usage and first edit, recorded in batches, and a turn's
//! usage is handed to the run.

use orchester_core::{connection::stream::AgentEvent, store::ActivityStore, time::Timestamp};
use orchester_ipc::Event;
use orchester_store::StoreError;

use super::{FLUSH_BATCH, Watch, Worker, WorkerEvent};

impl Worker {
  /// Takes in one event: a turn's usage goes to the run, and a new turn after a used-up budget
  /// ends the agent.
  pub(in super::super) fn observe(&self, watch: &mut Watch, mut event: AgentEvent) -> Result<(), StoreError> {
    if self.app_server().is_some() || !self.mcp_secrets.is_empty() {
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
