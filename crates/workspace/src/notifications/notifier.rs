//! [`Notifier`]: which of the events read show as OS notifications, and when.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
  time::Duration,
};

use orchester_core::{id::ProjectId, time::Timestamp};

use super::{COALESCE, Event, Kind, NotificationGroup, NotificationSettings, Target, words::Notification};

/// Whose events one reading holds: one project's, its Orchestrator conversations' proposals, which are read apart
/// from it, or the agent accounts' plan limits, which every project shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
  Project(ProjectId),
  Chats(ProjectId),
  Accounts,
}

/// What decides, at a moment, whether a notification is shown.
pub struct View<'a> {
  pub now: Timestamp,
  /// The minute of the local day, for the quiet hours.
  pub minute: u16,
  /// Whether the app's window has the focus.
  pub focused: bool,
  /// Whether the active tab shows a place now.
  pub on_screen: &'a dyn Fn(&Target) -> bool,
  pub settings: NotificationSettings,
}

/// An event still to be shown.
struct Pending {
  scope: Scope,
  event: Event,
  /// When it was first read.
  since: Timestamp,
  /// How many more events of its batch came with it.
  others: usize,
}

/// Decides the notifications of the events [`Self::observe`] is given, reading after reading. An event shows once,
/// when it is new in a reading of its scope: what stood at the first reading of a scope (what waited when the app
/// started, or when a project was opened) shows nothing. It is dropped before it shows when it stops standing (the
/// person or the Orchestrator dealt with it), when its group is switched off or the hours are quiet, and when the
/// window has the focus and shows the place it opens. A group's first event shows at once; those that come within
/// [`COALESCE`] of it wait for its end and show as one. An interruption waits [`COALESCE`] in any case: the
/// Orchestrator may continue the run by itself (ORC-53), and then there is nothing to tell. So does a merge: its
/// chat's record of a merge it carried out itself may be read after the run reads merged, and then the merge stops
/// being an event. The badge counts what *Needs you* counts in each open project.
#[derive(Default)]
pub struct Notifier {
  /// The keys of each scope's events at its last reading, each with its batch.
  standing: HashMap<Scope, HashMap<Arc<str>, Option<Arc<str>>>>,
  pending: Vec<Pending>,
  /// Batches already shown, kept while one of their events stands: the next runs the daily budget stops that day
  /// are no new notification.
  batches: HashSet<Arc<str>>,
  /// When each group last showed a notification.
  last: HashMap<NotificationGroup, Timestamp>,
  needs_you: HashMap<ProjectId, usize>,
}

impl Notifier {
  /// Takes the events standing in `scope`, read at `now`.
  pub fn observe(&mut self, scope: Scope, events: Vec<Event>, now: Timestamp) {
    let standing: HashMap<Arc<str>, Option<Arc<str>>> = events
      .iter()
      .map(|event| (event.key.clone(), event.batch.clone()))
      .collect();
    let Some(before) = self.standing.insert(scope, standing) else {
      self.batches.extend(events.into_iter().filter_map(|event| event.batch));
      return;
    };
    let standing = &self.standing[&scope];
    self
      .pending
      .retain(|pending| pending.scope != scope || standing.contains_key(&pending.event.key));
    for event in events {
      if !before.contains_key(&event.key) {
        self.add(scope, event, now);
      }
    }
    self.keep_standing_batches();
  }

  /// Forgets `scope`: a project the app no longer has open, with its conversations' scope.
  pub fn forget(&mut self, scope: Scope) {
    let mut gone = vec![scope];
    if let Scope::Project(project) = scope {
      self.needs_you.remove(&project);
      gone.push(Scope::Chats(project));
    }
    for scope in &gone {
      self.standing.remove(scope);
    }
    self.pending.retain(|pending| !gone.contains(&pending.scope));
    self.keep_standing_batches();
  }

  /// How many things wait for the person in `project` now, as its *Needs you* counts them.
  pub fn set_needs_you(&mut self, project: ProjectId, count: usize) {
    self.needs_you.insert(project, count);
  }

  /// What the taskbar's or the Dock's badge counts: what waits for the person in every open project.
  pub fn badge(&self) -> usize {
    self.needs_you.values().sum()
  }

  /// The notifications to show now, at most one per group; drops what `view` says is no use.
  pub fn due(&mut self, view: &View) -> Vec<Notification> {
    let quiet = view.settings.quiet_at(view.minute);
    self.pending.retain(|pending| {
      let event = &pending.event;
      view.settings.shows(event.group()) && !quiet && !(view.focused && (view.on_screen)(&event.target))
    });
    let mut shown = Vec::new();
    for group in NotificationGroup::ALL {
      if self.waits(group, view.now) {
        continue;
      }
      let (ready, rest): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut self.pending)
        .into_iter()
        .partition(|pending| pending.event.group() == group && settled(pending, view.now));
      self.pending = rest;
      if ready.is_empty() {
        continue;
      }
      self
        .batches
        .extend(ready.iter().filter_map(|pending| pending.event.batch.clone()));
      self.last.insert(group, view.now);
      let events: Vec<(&Event, usize)> = ready.iter().map(|pending| (&pending.event, pending.others)).collect();
      shown.push(Notification::of(group, &events, view.settings.sound));
    }
    shown
  }

  /// How long until [`Self::due`] may have something to show; `None` while nothing waits.
  pub fn next_due(&self, now: Timestamp) -> Option<Duration> {
    self
      .pending
      .iter()
      .map(|pending| {
        let group = self
          .last
          .get(&pending.event.group())
          .map_or(Duration::ZERO, |last| COALESCE.saturating_sub(after(*last, now)));
        let own = held(pending).map_or(Duration::ZERO, |hold| hold.saturating_sub(after(pending.since, now)));
        group.max(own)
      })
      .min()
  }

  /// Whether `group` showed a notification less than [`COALESCE`] ago.
  fn waits(&self, group: NotificationGroup, now: Timestamp) -> bool {
    self.last.get(&group).is_some_and(|last| after(*last, now) < COALESCE)
  }

  fn add(&mut self, scope: Scope, event: Event, now: Timestamp) {
    if let Some(batch) = &event.batch {
      if self.batches.contains(batch) {
        return;
      }
      if let Some(pending) = self
        .pending
        .iter_mut()
        .find(|pending| pending.event.batch.as_ref() == Some(batch))
      {
        pending.others += 1;
        return;
      }
    }
    self.pending.push(Pending {
      scope,
      event,
      since: now,
      others: 0,
    });
  }

  fn keep_standing_batches(&mut self) {
    let standing: HashSet<&Arc<str>> = self
      .standing
      .values()
      .flat_map(|keys| keys.values().flatten())
      .collect();
    self.batches.retain(|batch| standing.contains(batch));
  }
}

/// How long `pending` is held before it may show, from when it was read: an interruption or a merge [`COALESCE`].
fn held(pending: &Pending) -> Option<Duration> {
  matches!(pending.event.kind, Kind::Interrupted | Kind::Merged).then_some(COALESCE)
}

/// Whether `pending` was held long enough at `now`.
fn settled(pending: &Pending, now: Timestamp) -> bool {
  held(pending).is_none_or(|hold| after(pending.since, now) >= hold)
}

/// How long after `then` `now` is; none when it is not after it.
fn after(then: Timestamp, now: Timestamp) -> Duration {
  Duration::from_millis(u64::try_from(now.unix_millis() - then.unix_millis()).unwrap_or(0))
}
