//! Which OS notifications a person wants from the app (`docs/design/orchester-workspace.md`, "Notifications"), kept
//! with the application's settings: a switch per group, quiet hours and sound.

use thiserror::Error;

/// The groups notifications come in, each switched on or off as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NotificationGroup {
  /// Something waits for the person: a tool request, a question, a plan, a decision. On until turned off.
  NeedsYou,
  /// A run stopped at a limit or was interrupted, the project's runner is offline, a merge failed. On until turned
  /// off.
  Stopped,
  /// A run is ready to merge, was merged or failed, a plan limit, a proposal of the Orchestrator. Off until turned
  /// on.
  Finished,
}

impl NotificationGroup {
  pub const ALL: [Self; 3] = [Self::NeedsYou, Self::Stopped, Self::Finished];
}

/// Minutes in a day: a time of day is a minute after local midnight below it.
pub const MINUTES_PER_DAY: u16 = 24 * 60;

/// A span of local time each day with no OS notifications, from `start` up to `end`, in minutes after midnight; it
/// runs over midnight when `end` comes before `start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuietHours {
  start: u16,
  end: u16,
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
#[error("quiet hours need two different times of day")]
pub struct InvalidQuietHours;

impl QuietHours {
  /// From `start` to `end`, each a minute of the day; refused when either is not, or when both are the same.
  pub fn new(start: u16, end: u16) -> Result<Self, InvalidQuietHours> {
    if start >= MINUTES_PER_DAY || end >= MINUTES_PER_DAY || start == end {
      return Err(InvalidQuietHours);
    }
    Ok(Self { start, end })
  }

  pub const fn start(self) -> u16 {
    self.start
  }

  pub const fn end(self) -> u16 {
    self.end
  }

  /// Whether `minute` of the day is in the span: its start is, its end is not.
  pub const fn contains(self, minute: u16) -> bool {
    if self.start < self.end {
      self.start <= minute && minute < self.end
    } else {
      minute >= self.start || minute < self.end
    }
  }
}

impl Default for QuietHours {
  /// 22:00 to 08:00, what quiet hours start at when a person turns them on.
  fn default() -> Self {
    Self {
      start: 22 * 60,
      end: 8 * 60,
    }
  }
}

/// What OS notifications the app shows: the groups switched on, when it is quiet, and whether they make a sound. The
/// taskbar's or Dock's count of what needs the person shows whatever these say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotificationSettings {
  pub needs_you: bool,
  pub stopped: bool,
  pub finished: bool,
  /// No OS notifications in this span each day; `None`: none is quiet.
  pub quiet: Option<QuietHours>,
  pub sound: bool,
}

impl Default for NotificationSettings {
  fn default() -> Self {
    Self {
      needs_you: true,
      stopped: true,
      finished: false,
      quiet: None,
      sound: true,
    }
  }
}

impl NotificationSettings {
  /// Whether `group` is switched on.
  pub const fn shows(&self, group: NotificationGroup) -> bool {
    match group {
      NotificationGroup::NeedsYou => self.needs_you,
      NotificationGroup::Stopped => self.stopped,
      NotificationGroup::Finished => self.finished,
    }
  }

  /// `self` with `group` switched `on`.
  #[must_use]
  pub const fn with(mut self, group: NotificationGroup, on: bool) -> Self {
    match group {
      NotificationGroup::NeedsYou => self.needs_you = on,
      NotificationGroup::Stopped => self.stopped = on,
      NotificationGroup::Finished => self.finished = on,
    }
    self
  }

  /// Whether `minute` of the local day is in the quiet hours.
  pub fn quiet_at(&self, minute: u16) -> bool {
    self.quiet.is_some_and(|quiet| quiet.contains(minute))
  }
}
