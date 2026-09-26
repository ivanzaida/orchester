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
/// its first place, and one [`Event::AgentActivity`] per run with the highest sequence. Replies
/// and the other events are kept as they are.
pub fn coalesce(events: Vec<Event>) -> Vec<Event> {
  let mut kept: Vec<Event> = Vec::with_capacity(events.len());
  let mut changed: HashSet<RunId> = HashSet::new();
  let mut activity: HashMap<RunId, usize> = HashMap::new();
  for event in events {
    match event {
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
