//! [`NotificationsHost`]: mounted once by the session's window. It reads every open project through a
//! [`ProjectNotices`] each, and the accounts' plan limits itself; asks the notifier what is due whenever a reading,
//! the window's focus, the active tab or the settings change, or the notifier's next moment comes; shows that
//! through the system; opens the place of a clicked notification, in the tab that shows it or a new one, and brings
//! the window to the front; and keeps the *Needs you* count on the app's icon.

use std::{collections::VecDeque, sync::Arc, time::Duration};

use chrono::Timelike;
use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  node::Element,
};
use orchester_core::{connection::plan_limits::LimitReading, id::ProjectId, time::Timestamp};
use orchester_workspace::{
  background,
  messages::{self, Message},
  notifications::{NotificationSettings, Target, View, sources},
  tasks::attention::SummaryCard,
};
use parking_lot::Mutex;

use super::{
  NotificationPrefs, NotifierLink,
  os::{self, Clicks, Delivery, Shown},
  project::{ProjectNotices, ProjectProps},
};
use crate::{
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
    press::OpenIn,
  },
  project::Projects,
  routes::Route,
  services::{
    Services,
    accounts::{self, account_name},
    plan_limits,
  },
  ui::message::message_text,
};

/// How many shown notifications are remembered to open the decision card their click points at.
const REMEMBERED: usize = 32;
/// How often the host looks again while nothing waits: the quiet hours end, and a plan window resets, by the clock.
const IDLE: Duration = Duration::from_secs(60);
/// The least the host waits before it looks again.
const LEAST: Duration = Duration::from_secs(1);

/// What the host keeps between renders.
#[derive(Default)]
struct Kept {
  /// The projects read last.
  projects: Vec<ProjectId>,
  /// The count on the icon now.
  badge: Option<usize>,
  /// The notifications shown last, by the route their click opens.
  shown: VecDeque<(String, Target)>,
  /// Whether the stored settings were taken.
  loaded: bool,
}

pub struct NotificationsHost {
  link: NotifierLink,
  clicks: Clicks,
  delivery: Delivery,
  kept: Mutex<Kept>,
}

impl Component for NotificationsHost {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let clicks = Clicks::new(ctx.signal(0));
    let delivery = Delivery::of_build();
    match delivery {
      Delivery::Shows(id) => {
        if let Err(error) = os::start(id, &ctx.i18n().t("shell.app.name"), clicks.clone()) {
          tracing::warn!(%error, "the system takes no notifications from the app");
        }
      }
      Delivery::Silent => tracing::info!(
        variable = os::VARIABLE,
        "this build shows no OS notifications; only the installed app does unless the variable is 1"
      ),
    }
    Self {
      link: NotifierLink::new(ctx.signal(0)),
      clicks,
      delivery,
      kept: Mutex::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let prefs = NotificationPrefs::of(ctx);
    self.load(ctx, &prefs);
    let settings = prefs.get();
    let navigation = Navigation::of(ctx);
    for route in self.clicks.take() {
      self.open(&navigation, &route);
    }
    let _clicked = self.clicks.count();
    let _read = self.link.readings();
    let readers = self.readers(ctx, &navigation);
    self.read_accounts(ctx);
    self.show_due(ctx, &navigation, settings);
    self.badge(ctx);
    let wait = self.link.next_due(Timestamp::now()).unwrap_or(IDLE);
    wake_after(ctx, wait.max(LEAST));
    Column::new().with_children(readers)
  }
}

impl NotificationsHost {
  /// Takes the stored settings once they are read.
  fn load(&self, ctx: &mut Ctx, prefs: &NotificationPrefs) {
    let workspace = Services::of(ctx).workspace;
    let stored = ctx
      .future((), move |()| {
        let workspace = workspace.clone();
        async move {
          background::run(move || workspace.notification_settings())
            .await?
            .map(super::Chosen)
            .map_err(|error| -> Message {
              tracing::error!(%error, "the notification settings cannot be read; the defaults hold");
              messages::store_error(&error)
            })
        }
      })
      .state()
      .get()
      .data;
    let mut kept = self.kept.lock();
    if let Some(stored) = stored
      && !kept.loaded
    {
      kept.loaded = true;
      prefs.set(stored.0);
    }
  }

  /// A reader per project the tabs show that is open; the notifier forgets the others.
  fn readers(&self, ctx: &mut Ctx, navigation: &Navigation) -> Vec<Element> {
    let projects = Projects::of(ctx);
    let mut shown: Vec<ProjectId> = Vec::new();
    for project in navigation.tabs().tabs().iter().filter_map(|tab| tab.route().project()) {
      if !shown.contains(&project) {
        shown.push(project);
      }
    }
    let open: Vec<_> = shown
      .iter()
      .filter_map(|project| projects.state(*project)?.open().cloned())
      .collect();
    let ids: Vec<ProjectId> = open.iter().map(|open| open.project.id()).collect();
    {
      let mut kept = self.kept.lock();
      for gone in kept.projects.iter().filter(|project| !ids.contains(project)) {
        self.link.forget(*gone);
      }
      kept.projects = ids;
    }
    open
      .into_iter()
      .map(|open| {
        let key = format!("notices-{}", open.project.id());
        let props = ProjectProps {
          open,
          link: self.link.clone(),
        };
        ctx.mount_keyed::<ProjectNotices>(&key, props)
      })
      .collect()
  }

  /// The accounts' plan windows at 90 % or reset, by the accounts' names.
  fn read_accounts(&self, ctx: &mut Ctx) {
    let readings = plan_limits::readings(ctx);
    let listed = accounts::listed(ctx);
    let name = |reading: &LimitReading| -> Arc<str> {
      let account = reading.account_id().and_then(|id| listed.of(id));
      match account {
        Some(account) => account_name(ctx, account).into(),
        None => reading.provider.program_name().unwrap_or_default().into(),
      }
    };
    self
      .link
      .read_accounts(sources::of_plan_limits(&readings, Timestamp::now(), name));
  }

  /// Shows what the notifier says is due now, as the window and the settings stand.
  fn show_due(&self, ctx: &mut Ctx, navigation: &Navigation, settings: NotificationSettings) {
    let focused = ctx.window().is_focused;
    let active = navigation.tabs().active().route().clone();
    let on_screen = |target: &Target| match target {
      Target::Runner(project) => active.project() == Some(*project),
      other => Route::parse(&other.path()).is_ok_and(|route| route == active),
    };
    let local = chrono::Local::now();
    let minute = u16::try_from(local.hour() * 60 + local.minute()).expect("a minute of the day fits in u16");
    let view = View {
      now: Timestamp::now(),
      minute,
      focused,
      on_screen: &on_screen,
      settings,
    };
    for notification in self.link.due(&view) {
      let route = notification.target.path();
      let shown = Shown {
        title: message_text(ctx, &notification.title).to_string(),
        body: message_text(ctx, &notification.body).to_string(),
        route: route.clone(),
        sound: notification.sound,
      };
      if self.delivery == Delivery::Silent {
        tracing::debug!(route, "a notification this build does not show");
        continue;
      }
      if let Err(error) = os::show(&shown) {
        tracing::warn!(%error, "the system did not take a notification");
        continue;
      }
      let mut kept = self.kept.lock();
      kept.shown.push_back((route, notification.target));
      if kept.shown.len() > REMEMBERED {
        kept.shown.pop_front();
      }
    }
  }

  /// Puts what *Needs you* counts in every open project on the app's icon, when it changed.
  fn badge(&self, ctx: &Ctx) {
    let count = self.link.badge();
    let mut kept = self.kept.lock();
    if kept.badge == Some(count) {
      return;
    }
    let key = if count == 1 {
      "shell.notifications.badge.one"
    } else {
      "shell.notifications.badge.other"
    };
    let label = ctx.t_args(key, [("count", count.to_string())]);
    // Tried once per count: a refusal is logged, and the next count tries again.
    if let Err(error) = os::badge(count, &label) {
      tracing::warn!(%error, "the app's icon does not show what needs you");
    }
    kept.badge = Some(count);
  }

  /// Opens the place of a clicked notification: in the tab that shows it, else in a new tab, pointing at its
  /// decision card; and brings the window to the front.
  fn open(&self, navigation: &Navigation, path: &str) {
    let route = match Route::parse(path) {
      Ok(route) => route,
      Err(error) => {
        tracing::warn!(%error, path, "a notification's place is no route of the app");
        return;
      }
    };
    let target = self
      .kept
      .lock()
      .shown
      .iter()
      .find(|(shown, _)| shown == path)
      .map(|(_, target)| target.clone());
    let tabs = navigation.tabs_now();
    let at = match tabs.tabs().iter().find(|tab| *tab.route() == route) {
      Some(tab) => {
        navigation.select(tab.id());
        OpenIn::ActiveTab
      }
      None => OpenIn::NewTab,
    };
    match target.as_ref().and_then(pointed) {
      Some(card) => navigation.open_to_card(&route, at, card),
      None => navigation.open(route, at),
    }
    #[cfg(windows)]
    if crate::startup::front::bring_own_to_front() == crate::startup::front::Front::NoAnswer {
      tracing::info!("Windows kept the window where it was after a notification's click");
    }
  }
}

/// The decision card of a task's Summary `target` points at.
fn pointed(target: &Target) -> Option<Pointed> {
  let Target::Task {
    project,
    key,
    card: Some(card),
    ..
  } = target
  else {
    return None;
  };
  Some(Pointed {
    project: *project,
    task: key.clone(),
    card: match card {
      SummaryCard::Plan => PointedCard::Plan,
      SummaryCard::RaiseLimit => PointedCard::RaiseLimit,
      SummaryCard::Retries => PointedCard::Retries,
    },
  })
}

/// Renders the host again after `wait`.
fn wake_after(ctx: &mut Ctx, wait: Duration) {
  let at = Timestamp::now().unix_millis() + i64::try_from(wait.as_millis()).unwrap_or(i64::MAX);
  let _woken = ctx
    .future(at, move |_| async move {
      tokio::time::sleep(wait).await;
      Ok::<(), Message>(())
    })
    .state()
    .get();
}
