//! The Versions tab (`02c · versions` `josCI`): the draft, when there is one, and the published versions, newest
//! first with *Latest*; a press opens a version read-only over the flow (`super::viewing`). Reading them, a failure
//! and none yet keep today's words (`05b` `eCrOM`). Published versions never change.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  background, messages,
  pipelines::history::{PublishedVersion, VersionHistory},
};

use super::{
  parts,
  state::{Editing, State, View},
};
use crate::{
  project::ShownProject,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    message::message_text,
    pressable::Press,
    row_group::{GroupRow, RowGroup, note, tag},
    state::loading,
  },
};

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub state: State,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
  }
}
impl DevtoolsInspectable for Props {}

pub struct Versions {
  retry: Signal<u64>,
}
impl Component for Versions {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    ctx.props::<Props>().state.update(|s| {
      s.comparing = false;
      s.restore_requested = false;
    });
    Self { retry: ctx.signal(0) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let editing = props.state.get();
    let loaded = read(ctx, &props.project, &editing, self.retry.get());
    let mut pane = parts::pane("pipeline-versions");
    match loaded.data.filter(|_| loaded.error.is_none()) {
      Some(history) => {
        if editing.has_draft() {
          pane = pane.child(RowGroup::new("pipeline-versions-draft", ctx.t("pipelines.draftLabel")).row(draft_row(ctx, &props.state)));
        }
        pane = pane.child(published(ctx, &props.state, &history));
      }
      None if loaded.error.is_none() => {
        pane = pane.child(loading("pipeline-history-loading", &ctx.t("pipelines.reading")));
      }
      None => {}
    }
    if let Some(error) = loaded.error {
      let retry = self.retry.clone();
      pane = pane
        .child(field_error("pipeline-history-error", &message_text(ctx, &error)))
        .child(
          ActionButton::new("pipeline-history-retry", ButtonKind::Secondary, ctx.t("pipelines.reload"))
            .on_click(move || retry.update(|n| *n += 1)),
        );
    }
    pane.child(note("pipeline-versions-pinned", &ctx.t("pipelines.pinnedHint")))
  }
}

/// The pipeline's versions, read again after each write and *Try again*.
pub fn read(
  ctx: &mut Ctx,
  project: &ShownProject,
  editing: &Editing,
  retry: u64,
) -> lurq::app::ctx::FutureState<VersionHistory, messages::Message> {
  ctx
    .future(
      (project.open.repo_root.clone(), editing.opened.id, editing.epoch, editing.opened.latest.as_ref().map(|v| v.number), retry),
      |(root, id, ..)| async move {
        background::run(move || VersionHistory::load_in(&root, id).map_err(|e| messages::store_error(&e))).await?
      },
    )
    .state()
    .get()
}

fn published(ctx: &Ctx, state: &State, history: &VersionHistory) -> RowGroup {
  let versions = history.versions();
  let group = RowGroup::new("pipeline-versions-published", ctx.t("pipelines.groups.published")).count(versions.len().to_string());
  if versions.is_empty() {
    return group.lead(ctx.t("pipelines.historyEmpty"));
  }
  let latest = history.latest().map(|v| v.number);
  group.rows(versions.iter().map(|version| version_row(ctx, state, version, latest == Some(version.number))))
}

fn version_row(ctx: &Ctx, state: &State, version: &PublishedVersion, latest: bool) -> GroupRow {
  let number = version.number;
  let row = GroupRow::new(
    format!("pipeline-version-{}", number.get()),
    ctx.t_args("pipelines.version", [("version", number.get().to_string())]),
  )
  .hint(meta(ctx, version, "pipelines.historyMeta", "pipelines.historyFirstMeta"));
  let row = if latest {
    row.tag(tag("pipeline-latest-badge", &ctx.t("pipelines.latest"), false))
  } else {
    row
  };
  let (target, snapshot) = (state.clone(), version.body.clone());
  row.opens_page(Press::run(move || {
    // Shown at once; the route, which says what is viewed, follows.
    target.update(|s| {
      s.viewing = Some((*snapshot).clone());
      s.viewing_version = Some(number);
      s.comparing = false;
      s.restore_requested = false;
      s.epoch += 1;
    });
    target.get_untracked().views.view(number);
  }))
}

/// A version's age, its changes and its runs, in the words of `key`, or of `first` for the first version.
pub fn meta(ctx: &Ctx, version: &PublishedVersion, key: &str, first: &str) -> std::sync::Arc<str> {
  let changes = version.changes.as_ref().map_or(0, |c| c.changes.len());
  ctx.t_args(
    if version.changes.is_none() { first } else { key },
    [
      ("age", super::library::age(ctx, version.published_at).to_string()),
      ("changes", counted(ctx, changes, ("pipelines.changeOne", "pipelines.changesCount"))),
      ("runs", counted(ctx, version.runs.len(), ("pipelines.runOne", "pipelines.runsCount"))),
    ],
  )
}

/// `count` in the words of `one` for one and of `many` otherwise.
fn counted(ctx: &Ctx, count: usize, (one, many): (&str, &str)) -> String {
  if count == 1 {
    ctx.t(one).to_string()
  } else {
    ctx.t_args(many, [("count", count.to_string())]).to_string()
  }
}

/// The draft's row, which leads back to editing it.
fn draft_row(ctx: &Ctx, state: &State) -> GroupRow {
  let editing = state.get();
  let version = orchester_workspace::pipelines::write::next_version(&editing.opened)
    .map_or_else(String::new, |v| v.get().to_string());
  let hint = match editing.opened.draft_saved_at {
    Some(at) => ctx.t_args(
      "pipelines.draftRowHint",
      [
        ("draft", super::summary::draft(ctx, &editing).to_string()),
        ("saved", ctx.t_args("pipelines.draftSavedAt", [("age", super::library::age(ctx, at).to_string())]).to_string()),
      ],
    ),
    None => super::summary::draft(ctx, &editing),
  };
  let views = editing.views;
  GroupRow::new(
    "pipeline-history-draft",
    ctx.t_args("pipelines.draftVersion", [("version", version)]),
  )
  .hint(hint)
  .press(Press::run(move || views.tab(View::Steps)))
}
