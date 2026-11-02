//! The window's and each open project's reads of the settings, ahead of any tab: [`AppReads`] observes the
//! application's settings from the session's start, [`ProjectReads`] a project's from the moment it is open, so the
//! first visit to a tab finds them read. Both turn the changes they follow into invalidations ([`super::changes`]);
//! an observed entry then reads again behind its last value, whether or not a tab shows it. They render nothing.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, StreamEmitter},
  },
  node::Element,
};
use orchester_workspace::{
  app_updates::{AppUpdates, UpdateState},
  tasks::outside::{AppOutsideCounts, OutsideChanges, OutsideCounts},
  workspace::{OpenProject, OrchestratorConsent, Workspace},
};
use parking_lot::Mutex;

use super::{
  Session, app,
  changes::{self, APP_OUTSIDE, OUTSIDE},
  project,
};
use crate::{
  project::Projects,
  screens::settings::{Consent, orchester},
};

/// The application's change counts as the window last saw them.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct AppSeen {
  outside: AppOutsideCounts,
  accounts: u64,
  updates: u64,
}

/// Reads the application's settings from the session's start and keeps them current. Mounted once by the session's
/// window.
pub struct AppReads {
  seen: Mutex<Option<AppSeen>>,
}

impl Component for AppReads {
  type Props = ();

  fn create(_: &mut Ctx) -> Self {
    Self { seen: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let session = Session::of(ctx);
    let services = session.0.clone();
    let now = AppSeen {
      outside: app_outside(ctx, &services.changes),
      accounts: account_changes(ctx, &services.workspace),
      updates: update_changes(ctx, &services.updates),
    };
    let before = self.seen.lock().replace(now);
    if let Some(before) = before {
      let client = ctx.query_client();
      let invalidate = |reads: &[changes::AppRead]| {
        for read in reads {
          changes::invalidate_app(&client, &session, *read);
        }
      };
      for kind in APP_OUTSIDE {
        if now.outside.of(kind) != before.outside.of(kind) {
          invalidate(changes::app_touched(kind));
        }
      }
      if now.accounts != before.accounts {
        invalidate(changes::ACCOUNTS);
      }
      if now.updates != before.updates {
        invalidate(changes::UPDATES);
      }
    }
    prefetch_app(ctx, &session);
    Element::new()
  }
}

/// Observes every application setting a Settings tab shows, and the Orchester Cloud tab's account and servers.
fn prefetch_app(ctx: &mut Ctx, session: &Session) {
  ctx.query(app::usage_threshold(session.clone()));
  ctx.query(app::error_reports(session.clone()));
  ctx.query(app::update_settings(session.clone()));
  ctx.query(app::places(session.clone()));
  ctx.query(app::daily_report(session.clone()));
  ctx.query(app::browser_settings(session.clone()));
  ctx.query(app::agent_accounts(session.clone()));
  for consent in Consent::ALL {
    ctx.query(app::automation_consent(session.clone(), consent));
  }
  ctx.query(orchester::cloud_snapshot(session.clone()));
}

/// How often each application setting changed.
fn app_outside(ctx: &mut Ctx, changes: &OutsideChanges) -> AppOutsideCounts {
  ctx
    .stream(
      changes.clone(),
      |changes, emitter: StreamEmitter<AppOutsideCounts, ()>| async move {
        let mut updates = changes.subscribe_app();
        loop {
          let counts = *updates.borrow_and_update();
          if !emitter.emit(counts) || updates.changed().await.is_err() {
            return;
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// How often the person's accounts changed.
fn account_changes(ctx: &mut Ctx, workspace: &Workspace) -> u64 {
  ctx
    .stream(
      workspace.clone(),
      |workspace, emitter: StreamEmitter<u64, ()>| async move {
        let mut changes = workspace.account_changes();
        loop {
          let count = *changes.borrow_and_update();
          if !emitter.emit(count) || changes.changed().await.is_err() {
            return;
          }
        }
      },
    )
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// How often what is stored of the update may have changed: a setting saved, or a state of another kind, such as a
/// check's end. A download's progress is not one: it would read the database with every chunk.
fn update_changes(ctx: &mut Ctx, updates: &AppUpdates) -> u64 {
  ctx
    .stream(updates.clone(), |updates, emitter: StreamEmitter<u64, ()>| async move {
      let (mut states, mut saved) = (updates.subscribe(), updates.settings_saved());
      let kind = |state: &UpdateState| std::mem::discriminant(state);
      let mut last_kind = kind(&states.borrow());
      let mut kinds = 0;
      loop {
        let now = kind(&states.borrow_and_update());
        if now != last_kind {
          last_kind = now;
          kinds += 1;
        }
        if !emitter.emit(kinds + *saved.borrow_and_update()) {
          return;
        }
        tokio::select! {
          changed = states.changed() => if changed.is_err() { return },
          changed = saved.changed() => if changed.is_err() { return },
        }
      }
    })
    .state()
    .get()
    .data
    .unwrap_or_default()
}

/// The open project whose settings [`ProjectReads`] reads.
#[derive(Clone, PartialEq)]
pub struct ProjectProps(pub OpenProject);

impl DevtoolsInspectable for ProjectProps {}

/// A project's change counts and its runs' state count, as its reads last saw them.
#[derive(Clone, Copy, PartialEq, Eq)]
struct ProjectSeen {
  outside: OutsideCounts,
  runs: u64,
}

/// Reads an open project's settings from the moment it is open and keeps them current: automation's recorded changes
/// and the screens' own, and its runs starting and ending, invalidate what they touched. Mounted by the project's
/// loader beside its feed.
pub struct ProjectReads {
  seen: Mutex<Option<ProjectSeen>>,
}

impl Component for ProjectReads {
  type Props = ProjectProps;

  fn create(_: &mut Ctx) -> Self {
    Self { seen: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ProjectProps(open) = ctx.props::<ProjectProps>().clone();
    let session = Session::of(ctx);
    let feed = Projects::of(ctx).feed(open.project.id());
    let now = ProjectSeen {
      outside: feed.outside.get(),
      runs: feed.states.get(),
    };
    let before = self.seen.lock().replace(now);
    if let Some(before) = before {
      let client = ctx.query_client();
      let root = &open.repo_root;
      let invalidate = |reads: &[changes::ProjectRead]| {
        for read in reads {
          changes::invalidate(&client, &session, root, *read);
        }
      };
      for kind in OUTSIDE {
        if now.outside.of(kind) != before.outside.of(kind) {
          invalidate(changes::touched(kind));
        }
      }
      if now.runs != before.runs {
        invalidate(changes::RUNS);
      }
    }
    prefetch_project(ctx, &session, &open);
    Element::new()
  }
}

/// Observes every setting of the project a Project settings tab shows.
fn prefetch_project(ctx: &mut Ctx, session: &Session, open: &OpenProject) {
  let (s, r) = (session, &open.repo_root);
  ctx.query(project::project_settings(s.clone(), r.clone()));
  ctx.query(project::repository(s.clone(), r.clone()));
  ctx.query(project::estimates(s.clone(), r.clone()));
  for consent in OrchestratorConsent::ALL {
    ctx.query(project::orchestrator_allows(s.clone(), r.clone(), consent));
  }
  ctx.query(project::memory_settings(s.clone(), r.clone()));
  ctx.query(project::variables(s.clone(), r.clone()));
  ctx.query(project::connections(s.clone(), r.clone()));
  ctx.query(project::secrets(s.clone(), r.clone(), None));
  ctx.query(project::project_sources(s.clone(), r.clone()));
  ctx.query(project::index_builds(s.clone(), r.clone()));
}
