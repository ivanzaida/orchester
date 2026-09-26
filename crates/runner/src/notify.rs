//! Change notifications for the desktop. They only say *that* something changed; the desktop reads
//! the change from the database, so notifications waiting to be sent can be coalesced.

use std::collections::{HashMap, HashSet};

use orchester_core::id::RunId;
use orchester_ipc::Event;
use tokio::sync::mpsc;

/// Where the runner's events go: the connected desktop.
#[derive(Clone)]
pub struct Notifier(mpsc::UnboundedSender<Event>);

impl Notifier {
  pub fn new(events: mpsc::UnboundedSender<Event>) -> Self {
    Self(events)
  }

  pub(crate) fn send(&self, event: Event) {
    // Without a desktop listening nobody needs the notification; the database has the change.
    if self.0.send(event).is_err() {
      tracing::debug!("no desktop is listening for notifications");
    }
  }
}

/// `events`, in order, with repeated notifications merged: one [`Event::RunChanged`] per run, at
/// its first place, one [`Event::AgentActivity`] per run with the highest sequence, and only the
/// last [`Event::WorkSummary`], which replaces the ones before it. Replies and the other events
/// are kept as they are.
pub fn coalesce(events: Vec<Event>) -> Vec<Event> {
  let last_summary = events.iter().rposition(|event| matches!(event, Event::WorkSummary(_)));
  let mut kept: Vec<Event> = Vec::with_capacity(events.len());
  let mut changed: HashSet<RunId> = HashSet::new();
  let mut activity: HashMap<RunId, usize> = HashMap::new();
  for (index, event) in events.into_iter().enumerate() {
    match event {
      Event::WorkSummary(_) if Some(index) != last_summary => {}
      Event::RunChanged { run, .. } => {
        if changed.insert(run) {
          kept.push(event);
        }
      }
      Event::AgentActivity { run, sequence } => match activity.get(&run) {
        Some(&index) => {
          if let Event::AgentActivity {
            sequence: kept_sequence,
            ..
          } = &mut kept[index]
          {
            *kept_sequence = (*kept_sequence).max(sequence);
          }
        }
        None => {
          activity.insert(run, kept.len());
          kept.push(event);
        }
      },
      other => kept.push(other),
    }
  }
  kept
}
