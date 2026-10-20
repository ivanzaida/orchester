//! OS notifications for what needs the person, what stopped and what finished (ORC-56,
//! `docs/design/orchester-workspace.md`, "Notifications"). Which ones show is decided in one place,
//! `orchester_workspace::notifications`; this module feeds it what the window follows of every open project
//! ([`project`]), shows what it decides through the operating system ([`os`]), opens the place a click names and keeps
//! the *Needs you* count on the app's icon ([`host`]). The person's choices (Settings › Automation, *Notifications*)
//! are shared through [`NotificationPrefs`].

mod host;
pub mod os;
mod project;

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal};
use orchester_core::{id::ProjectId, time::Timestamp};
pub use orchester_workspace::notifications::NotificationSettings;
use orchester_workspace::notifications::{Event, Notification, Notifier, Scope, View};
use parking_lot::Mutex;

pub use self::host::NotificationsHost;

/// The person's notification settings as the window holds them: read once by [`NotificationsHost`], changed by
/// Settings, which also saves them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Chosen(pub NotificationSettings);

impl lurq::app::component::DevtoolsInspectable for Chosen {}

/// Where the window keeps [`Chosen`]; the session's window provides it.
#[derive(Clone)]
pub struct NotificationPrefs(Signal<Chosen>);

impl PartialEq for NotificationPrefs {
  fn eq(&self, other: &Self) -> bool {
    self.0.id() == other.0.id()
  }
}

impl NotificationPrefs {
  pub fn new(settings: Signal<Chosen>) -> Self {
    Self(settings)
  }

  /// The prefs the session's window provides.
  pub fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the session's window provides the notification settings")
  }

  /// The settings; reading them in a render renders again when they change.
  pub fn get(&self) -> NotificationSettings {
    self.0.get().0
  }

  /// The settings, without subscribing a render to them: in a handler.
  pub fn get_now(&self) -> NotificationSettings {
    self.0.get_untracked().0
  }

  pub fn set(&self, settings: NotificationSettings) {
    if self.0.get_untracked().0 != settings {
      self.0.set(Chosen(settings));
    }
  }
}

/// The notifier the window's readers feed and its host asks, with a count of the readings that changed it.
#[derive(Clone)]
struct NotifierLink(Arc<Shared>);

struct Shared {
  notifier: Mutex<Notifier>,
  readings: Signal<u64>,
}

impl PartialEq for NotifierLink {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl lurq::app::component::DevtoolsInspectable for NotifierLink {}

impl NotifierLink {
  fn new(readings: Signal<u64>) -> Self {
    Self(Arc::new(Shared {
      notifier: Mutex::new(Notifier::default()),
      readings,
    }))
  }

  /// Takes a project's reading, `needs_you` the count its *Needs you* shows, and tells the host.
  fn read_project(&self, project: ProjectId, events: Vec<Event>, needs_you: usize) {
    {
      let mut notifier = self.0.notifier.lock();
      notifier.observe(Scope::Project(project), events, Timestamp::now());
      notifier.set_needs_you(project, needs_you);
    }
    self.0.readings.update(|count| *count += 1);
  }

  /// Takes the accounts' plan limits; the host reads them itself, so it is not told.
  fn read_accounts(&self, events: Vec<Event>) {
    self
      .0
      .notifier
      .lock()
      .observe(Scope::Accounts, events, Timestamp::now());
  }

  fn forget(&self, project: ProjectId) {
    self.0.notifier.lock().forget(Scope::Project(project));
  }

  /// How many readings changed it; reading it in a render renders again on the next.
  fn readings(&self) -> u64 {
    self.0.readings.get()
  }

  fn due(&self, view: &View) -> Vec<Notification> {
    self.0.notifier.lock().due(view)
  }

  fn next_due(&self, now: Timestamp) -> Option<std::time::Duration> {
    self.0.notifier.lock().next_due(now)
  }

  fn badge(&self) -> usize {
    self.0.notifier.lock().badge()
  }
}
