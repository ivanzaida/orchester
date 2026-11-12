//! A published version viewed read-only (`05 · viewing v2` `AaaH0`): a banner with `← Versions`, the version, its
//! age, changes and the runs pinned to it, and *Back to editing*; under it the same flow, read-only, each step
//! inspectable. With no step chosen the column lists the version's changes. A pipeline holds no draft (D-032), so a
//! version is neither restored as one nor compared with one.
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
use orchester_workspace::pipelines::history::PublishedVersion;

use super::{
  parts,
  state::{State, View},
};
use crate::{
  project::ShownProject,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    pressable::Press,
    row_group::{RowGroup, note},
    skeleton::{self, Skeleton},
  },
};

/// The version's changes' bones while it is read.
const CHANGE_BONES: [(f32, f32); 3] = [(180.0, 120.0), (150.0, 140.0), (200.0, 100.0)];

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
    let flow = super::flow::flow(ctx, &props.state, (busy, false));
    let column = if editing.view.step().is_some() || editing.view == View::Deliver {
      super::column::column(ctx, &props.state, props.locked)
    } else {
      match &version {
        Some(version) => changes_column(ctx, version),
        None => {
          let rows = skeleton::list(ctx, "pipeline-history-reading", &CHANGE_BONES);
          Skeleton::new("pipeline-history-loading", ctx.t("pipelines.reading")).show(ctx, rows)
        }
      }
    };
    Column::new()
      .id("pipeline-viewing")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .child(banner(ctx, &props.state, version.as_ref()))
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
fn banner(ctx: &Ctx, state: &State, version: Option<&PublishedVersion>) -> Row {
  let editing = state.get();
  let number = editing
    .viewing_version
    .map_or_else(String::new, |v| v.get().to_string());
  let meta = version.map_or_else(
    || ctx.t("pipelines.reading"),
    |version| super::versions::meta(ctx, version, "pipelines.viewingMeta", "pipelines.viewingFirstMeta"),
  );
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
    .child(
      ActionButton::new(
        "pipeline-back-draft",
        ButtonKind::Ghost,
        ctx.t("pipelines.backToEditing"),
      )
      .on_click(move || back_to_editing(&back)),
    )
}

/// *Back to editing*: the edit again, leaving no entry for the version.
pub fn back_to_editing(state: &State) {
  state.update(|s| {
    s.viewing = None;
    s.viewing_version = None;
    s.epoch += 1;
  });
  state.get_untracked().views.to_edit();
}

/// The version's changes and the runs pinned to it.
fn changes_column(ctx: &Ctx, version: &PublishedVersion) -> Element {
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
        group = group.row(parts::change_row(
          ctx,
          &format!("pipeline-version-change-{index}"),
          change,
        ));
      }
    }
  }
  Column::new()
    .id("pipeline-version-summary")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(group)
    .child(note("history-pinned", &ctx.t("pipelines.pinnedHint")))
    .into()
}
