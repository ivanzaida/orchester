//! Context › Sources (`Sources` `lWL4C` in `03 · Context`, empty in `09` `mE5oU`): the project's sources with whether
//! runs can read each now, *Add a folder or file* checked in place, what agents may do outside the worktree, and that
//! changes apply to runs that start later. Adding a source lets agents read more without asking, so it is asked in
//! place first (`connections::widen`, the window's input locked while it shows); removing one is taken at once, says
//! which running runs keep it, and offers Undo. The rules are `orchester_workspace::sources::project`.

mod view;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
  },
  core::Signal,
  node::Element,
};
use orchester_core::source::SourcePath;
use orchester_workspace::{
  background,
  messages::{self, Message},
  sources::project::{self as sources, Removed},
  tasks::outside::Outside,
};

use super::{Status, TabProps, kit::Group};
use crate::{
  mcp::input_gate::Guarded,
  queries::{
    self, Again, Session,
    project::{self, SourcesRead},
  },
  screens::connections::widen::{WidenConfirm, WidenLook, WidenProps},
  services::Services,
  ui::{add_source::Asked, field::field_error, message::message_text, skeleton::Skeleton, state::load_error},
};

/// The project's sources as read, where a new one is checked, and when they were read.
type Read = SourcesRead;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Change {
  Add(SourcePath),
  Remove(SourcePath),
}

impl DevtoolsInspectable for Change {}

/// What a change did: nothing to say for an added source; a removed one's notice.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Done(Option<Removed>);

impl DevtoolsInspectable for Done {}

type Save = FutureAction<(PathBuf, Change), Done, Message>;

#[derive(Clone)]
pub(super) struct Sources {
  save: Save,
  /// Counts *Check again* and *Try again*, each of which reads the sources again.
  revision: Signal<u64>,
  again: Arc<Again>,
  /// The source a person is asked about before it is added.
  asking: Signal<Option<Asked>>,
  /// The source removed last, until the next change.
  removed: Signal<Option<Removed>>,
  /// *Add* opened the path box under its row.
  adding: Signal<bool>,
}

impl Component for Sources {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: save_action(ctx),
      revision: ctx.signal(0),
      again: Arc::new(Again::live()),
      asking: ctx.signal(None),
      removed: ctx.signal(None),
      adding: ctx.signal(false),
    };
    let (after, status, feed) = (
      tab.clone(),
      ctx.props::<TabProps>().status.clone(),
      ctx.props::<TabProps>().project.feed.clone(),
    );
    // A change records itself, which reads the sources again (`queries`).
    ctx.watch(&tab.save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
        return;
      }
      if state.error.is_some() {
        status.set(Status::Failed);
      } else if let Some(Done(removed)) = &state.data {
        after.asking.set(None);
        if removed.is_none() {
          after.adding.set(false);
        }
        after.removed.set(removed.clone());
        status.set(Status::Saved);
        feed.saved();
      }
    });
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let state = self.read(ctx, &props);
    // A failed read shows its error, not the list read before, which could still be changed.
    let Some(read) = state.data.clone().filter(|_| !state.is_rejected()) else {
      return self.pending(ctx, &state);
    };
    if !self.save.state().get().is_pending() && self.save.state().get().error.is_none() {
      props.status.set(Status::Saved);
    }
    let busy = self.save.state().get().is_pending();
    let mut group = self.group(ctx, &props, &read, busy);
    if let Some(error) = self.save.state().get().error {
      group = group.after(field_error("settings-sources-save-error", &message_text(ctx, &error)));
    }
    group.into()
  }
}

impl Sources {
  /// The sources through their query, which the project's loader read as the project opened: the last read at once,
  /// read again behind it when the tab opens, after each change, at *Check again* and at a change elsewhere.
  fn read(&self, ctx: &mut Ctx, props: &TabProps) -> FutureState<Read, Message> {
    let session = Session::of(ctx);
    let query = ctx.query(project::project_sources(session, props.project.open.repo_root.clone()));
    self.again.follow(self.revision.get(), &query);
    queries::state(&query)
  }

  fn pending(&self, ctx: &mut Ctx, state: &FutureState<Read, Message>) -> Element {
    match state.error.as_ref().filter(|_| state.is_rejected()) {
      Some(error) => {
        let revision = self.revision.clone();
        load_error(
          ctx,
          "settings-sources-load-error",
          (
            &ctx.t("project_settings.sourcesTab.loadFailed"),
            &message_text(ctx, error),
          ),
          move || revision.update(|revision| *revision += 1),
        )
      }
      None => {
        let bones = view::bones(ctx);
        Skeleton::new("settings-sources-loading", ctx.t("project_settings.sourcesTab.reading")).show(ctx, bones)
      }
    }
  }

  /// The group: its count, the sources or that there is none, a removed source with Undo, the question about one to
  /// add, and *Add a folder or file*.
  fn group(&self, ctx: &mut Ctx, props: &TabProps, read: &Read, busy: bool) -> Group {
    let root = props.project.open.repo_root.clone();
    let remove = {
      let (save, root) = (self.save.clone(), root.clone());
      Arc::new(move |path: SourcePath| {
        if !save.state().get_untracked().is_pending() {
          save.run((root.clone(), Change::Remove(path)));
        }
      })
    };
    let check_again = {
      let revision = self.revision.clone();
      Arc::new(move || revision.update(|revision| *revision += 1))
    };
    let asking = self.asking.get();
    let mut group = view::heading(ctx, read).end(view::outside_info(ctx)).rows(view::rows(
      ctx,
      read,
      (remove, check_again),
      busy,
      &self.adding,
    ));
    if let Some(removed) = self.removed.get() {
      let (ask, path) = (self.asking.clone(), removed.path.clone());
      group = group.row(view::removed(ctx, &removed, move || {
        ask.set(Some(Asked(path.clone())));
      }));
    }
    let ask = self.asking.clone();
    if !read.listed.is_empty() || self.adding.get() {
      group = group.row(view::add(
        ctx,
        read,
        (busy || asking.is_some(), &self.adding),
        Arc::new(move |path| ask.set(Some(Asked(path)))),
      ));
    }
    if let Some(Asked(path)) = &asking {
      group = group.after(self.question(ctx, &root, path));
    }
    group
  }

  fn question(&self, ctx: &mut Ctx, root: &std::path::Path, path: &SourcePath) -> Element {
    let reasons = sources::widenings(path)
      .iter()
      .map(|reason| message_text(ctx, reason))
      .collect::<Vec<_>>();
    let (save, root, keep, path) = (self.save.clone(), root.to_path_buf(), self.asking.clone(), path.clone());
    ctx.mount_keyed::<WidenConfirm>(
      "settings-sources-confirm",
      WidenProps {
        id: "settings-sources-question".into(),
        look: WidenLook::Box,
        affirming: false,
        automation: Guarded::PermissionWidening,
        title: ctx.t("project_settings.sourcesTab.question"),
        reasons: reasons.into(),
        confirm: ctx.t("project_settings.sourcesTab.confirm"),
        keep: ctx.t("project_settings.sourcesTab.keep"),
        on_keep: Arc::new(move || keep.set(None)),
        on_confirm: Arc::new(move || {
          if !save.state().get_untracked().is_pending() {
            save.run((root.clone(), Change::Add(path.clone())));
          }
        }),
      },
    )
  }
}

fn save_action(ctx: &mut Ctx) -> Save {
  let services = Services::of(ctx);
  ctx.future_action(move |(root, change): (PathBuf, Change)| {
    let changes = services.changes.clone();
    async move {
      background::run(move || {
        // Only a person's confirmed question adds one: the question is the confirmation.
        let done = match &change {
          Change::Add(path) => sources::add_in(&root, path, true).map(|()| Done(None))?,
          Change::Remove(path) => Done(Some(sources::remove_in(&root, path)?)),
        };
        let db = orchester_store::ProjectDb::open(&root).map_err(|error| messages::store_error(&error))?;
        changes.record(db.project().id(), Outside::ProjectSettings);
        Ok(done)
      })
      .await?
    }
  })
}
