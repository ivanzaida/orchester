//! A published version viewed read-only (`05 · viewing v2` `AaaH0`): a banner with `← Versions`, the version, its
//! age, changes and the runs pinned to it, *Compare* (`05b` `eCrOM`), *Back to editing* and *Restore vN as draft*,
//! asked in place only while a draft exists; under it the same flow, read-only, each step inspectable. With no step
//! chosen the column lists the version's changes and, while comparing, how it differs from the draft.
use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{PipelineChange, changes};
use orchester_workspace::pipelines::{history::PublishedVersion, write::next_version};

use super::{
  parts,
  state::{Editing, State, View},
};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    pressable::Press,
    row_group::{RowGroup, note},
    state::loading,
  },
};

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub state: State,
  /// A question or a reload holds the page, and a write runs too.
  pub locked: (bool, bool),
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.locked == other.locked
  }
}
impl DevtoolsInspectable for Props {}

pub struct Viewed {
  retry: Signal<u64>,
}
impl Component for Viewed {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self { retry: ctx.signal(0) }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let editing = props.state.get();
    let loaded = super::versions::read(ctx, &props.project, &editing, self.retry.get());
    let version = loaded
      .data
      .as_ref()
      .and_then(|history| editing.viewing_version.and_then(|n| history.version(n)).cloned());
    let (_, busy) = props.locked;
    let flow = super::flow::flow(ctx, &props.state, busy);
    let column = if editing.view.step().is_some() || editing.view == View::Deliver {
      super::column::column(ctx, &props.state, props.locked)
    } else {
      match &version {
        Some(version) => changes_column(ctx, &editing, version),
        None => loading("pipeline-history-loading", &ctx.t("pipelines.reading")).into(),
      }
    };
    Column::new()
      .id("pipeline-viewing")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(banner(ctx, &props.state, version.as_ref(), busy))
      .with_children(editing.restore_requested.then(|| super::restore::question(ctx, &props.state, busy)))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Section)
          .align_items(Alignment::Start)
          .child(flow)
          .child(Column::new().flex(1.0).min_width(Dimension::Px(0.0)).child(column)),
      )
  }
}

/// The banner over the viewed flow (`Viewing` `Ykw38`).
fn banner(ctx: &Ctx, state: &State, version: Option<&PublishedVersion>, writing: bool) -> Row {
  let editing = state.get();
  let number = editing.viewing_version.map_or_else(String::new, |v| v.get().to_string());
  let meta = if editing.comparing {
    comparison(ctx, &editing)
  } else {
    version.map_or_else(
      || ctx.t("pipelines.reading"),
      |version| super::versions::meta(ctx, version, "pipelines.viewingMeta", "pipelines.viewingFirstMeta"),
    )
  };
  let views = editing.views;
  let back = state.clone();
  Row::new()
    .id("pipeline-viewing-banner")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .child(parts::back_link(
      "pipeline-back-versions",
      &ctx.t("pipelines.versions"),
      Press::run(move || views.tab(View::Versions)),
    ))
    .child(
      Text::new(&ctx.t_args("pipelines.version", [("version", number)]))
        .id("pipeline-viewing-version")
        .variant(ExtraTypography::Strong.role()),
    )
    .child(
      Text::new(&meta)
        .id("pipeline-viewing-meta")
        .variant(crate::theme::GroupText::Hint.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(compare_button(ctx, state))
    .child(
      ActionButton::new("pipeline-back-draft", ButtonKind::Ghost, ctx.t("pipelines.returnDraft"))
        .on_click(move || back_to_draft(&back)),
    )
    .child(restore_button(ctx, state, writing))
}

/// *Back to editing*: the draft again, leaving no entry for the version.
pub fn back_to_draft(state: &State) {
  state.update(|s| {
    s.viewing = None;
    s.viewing_version = None;
    s.comparing = false;
    s.restore_requested = false;
    s.epoch += 1;
  });
  state.get_untracked().views.to_draft();
}

fn restore_button(ctx: &Ctx, state: &State, writing: bool) -> ActionButton {
  let editing = state.get();
  let restore = state.clone();
  let button = ActionButton::new(
    "pipeline-restore",
    ButtonKind::Secondary,
    ctx.t_args(
      "pipelines.restoreVersion",
      [(
        "version",
        editing
          .viewing_version
          .map_or_else(String::new, |v| v.get().to_string()),
      )],
    ),
  )
  .icon(Icon::RotateCcw)
  .on_click(move || {
    if writing {
      return;
    }
    let editing = restore.get_untracked();
    if editing.has_draft() {
      restore.update(|s| s.restore_requested = true);
    } else {
      restore.update(Editing::restore);
      editing.views.to_draft();
    }
  });
  if writing { button.disabled() } else { button }
}

/// Where the comparison counts the Deliver step among the changed steps: no step key holds a space.
const DELIVER_KEY: &str = "deliver step";

/// *Compared with saved draft v4 · 2 steps differ*.
fn comparison(ctx: &Ctx, editing: &Editing) -> std::sync::Arc<str> {
  let count = editing.viewing.as_ref().map_or(0, |before| {
    changes(before, &editing.opened.body)
      .iter()
      .filter_map(|change| match change {
        PipelineChange::Step { key, .. } => Some(key.as_str()),
        PipelineChange::Deliver(_) => Some(DELIVER_KEY),
        PipelineChange::Description | PipelineChange::Delivers { .. } => None,
      })
      .collect::<std::collections::BTreeSet<_>>()
      .len()
  });
  ctx.t_args(
    if count == 1 {
      "pipelines.comparisonCountOne"
    } else {
      "pipelines.comparisonCount"
    },
    [
      ("count", count.to_string()),
      ("target", comparison_title(ctx, editing).to_string()),
    ],
  )
}

fn comparison_title(ctx: &Ctx, editing: &Editing) -> std::sync::Arc<str> {
  let (key, version) = if editing.opened.draft_saved_at.is_some() {
    ("pipelines.comparedSavedDraft", next_version(&editing.opened))
  } else {
    (
      "pipelines.comparedPublished",
      editing.opened.latest.as_ref().map(|v| v.number),
    )
  };
  ctx.t_args(
    key,
    [("version", version.map_or_else(String::new, |v| v.get().to_string()))],
  )
}

fn no_comparison_draft(editing: &Editing) -> bool {
  editing.opened.draft_saved_at.is_none() && editing.viewing_version == editing.opened.latest.as_ref().map(|v| v.number)
}

fn compare_button(ctx: &Ctx, state: &State) -> ActionButton {
  let editing = state.get();
  let compare = state.clone();
  let button = ActionButton::new(
    "pipeline-compare",
    ButtonKind::Ghost,
    ctx.t(if editing.comparing {
      "pipelines.hideComparison"
    } else if editing.opened.draft_saved_at.is_some() || no_comparison_draft(&editing) {
      "pipelines.compare"
    } else {
      "pipelines.comparePublished"
    }),
  )
  .icon(Icon::GitCompare)
  .on_click(move || compare.update(|s| s.comparing = !s.comparing));
  if editing.unsaved() || no_comparison_draft(&editing) {
    button.disabled()
  } else {
    button
  }
}

/// The version's changes, the runs pinned to it, and while comparing how it differs from the draft.
fn changes_column(ctx: &Ctx, editing: &Editing, version: &PublishedVersion) -> Element {
  let number = version.number.get().to_string();
  let mut group = RowGroup::new(
    "pipeline-version-changes",
    ctx.t_args("pipelines.changesIn", [("version", number)]),
  )
  .count(ctx.t_args("pipelines.runs", [("count", version.runs.len().to_string())]));
  match &version.changes {
    None => group = group.lead(ctx.t("pipelines.firstVersion")),
    Some(listed) => {
      for (index, change) in listed.changes.iter().enumerate() {
        group = group.row(parts::change_row(ctx, &format!("pipeline-version-change-{index}"), change));
      }
    }
  }
  let mut column = Column::new()
    .id("pipeline-version-summary")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(group);
  if editing.comparing {
    let rows = changes(&version.body, &editing.opened.body)
      .iter()
      .enumerate()
      .map(|(index, change)| parts::change_row(ctx, &format!("pipeline-comparison-change-{index}"), change))
      .collect::<Vec<_>>();
    column = column.child(RowGroup::new("pipeline-comparison-changes", comparison_title(ctx, editing)).rows(rows));
  }
  if editing.unsaved() {
    column = column.child(note("pipeline-compare-unavailable", &ctx.t("pipelines.compareUnavailable")));
  } else if no_comparison_draft(editing) {
    let next = next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string());
    column = column.child(note(
      "pipeline-history-no-draft",
      &ctx.t_args("pipelines.historyNoDraft", [("version", next)]),
    ));
  }
  column
    .child(note("history-pinned", &ctx.t("pipelines.pinnedHint")))
    .into()
}
