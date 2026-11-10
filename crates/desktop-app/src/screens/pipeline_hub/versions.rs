//! The list under *Versions* in a pipeline's head (`Versions menu` `zpuln` in `Pipelines · v2` P12 `nSr5q`): the
//! heading *Versions* with the pipeline's name, a row per published version, newest first, the current one with
//! *current* and a check, each saying when it was published and what still holds it (*used by 13 tasks* for the current
//! one, *2 runs on it* or *no runs on it* for an older one), and the foot *Runs keep the version they started on.* A
//! row under the pointer ends in *View ›*; pressing it closes the list and opens that version read-only in the panel
//! ([`super::version_view`]).
//!
//! The versions are read when the list opens, and again after the pipeline's latest version changes.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ResourceId, resource::VersionNumber, time::Timestamp};
use orchester_workspace::{
  background,
  messages::{self, Message},
  pipelines::history::{PublishedVersion, Use, VersionHistory},
};

use super::version_view::{Viewed, ViewedSignal};
use crate::{
  project::ShownProject,
  screens::team_hub::words::date,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    menu::{foot, menu_panel, separator},
    message::message_text,
    pressable::Press,
    skeleton::{self, Skeleton},
  },
};

/// The list's width (`Versions menu` `zpuln`).
pub(super) const WIDTH: f32 = 300.0;

/// The versions' bones while they are read: how wide each row's lines are.
const BONES: [(f32, f32); 3] = [(24.0, 190.0), (24.0, 180.0), (24.0, 170.0)];

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub pipeline: ResourceId,
  pub name: Arc<str>,
  /// The tasks that use the pipeline, which is what the current version says it is held by.
  pub tasks: usize,
  /// The pipeline's latest version, so the versions are read again when a save makes another.
  pub latest: Option<VersionNumber>,
  pub now: Timestamp,
  /// Whether the list is open; a choice closes it.
  pub open: Signal<bool>,
  /// The version the panel shows read-only; a row sets it.
  pub viewing: ViewedSignal,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (&self.project, self.pipeline, &self.name, self.tasks, self.latest)
      == (&other.project, other.pipeline, &other.name, other.tasks, other.latest)
  }
}
impl DevtoolsInspectable for Props {}

/// The list's panel for pipeline `props.pipeline`.
pub(super) fn list(ctx: &mut Ctx, props: Props) -> Element {
  ctx.mount_keyed::<Versions>(&format!("pipeline-hub-versions-{}", props.pipeline), props)
}

struct Versions {
  /// The number of the version under the pointer, whose row ends in *View ›*.
  hovered: Signal<Option<u32>>,
  retry: Signal<u64>,
}

impl Component for Versions {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(None),
      retry: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let root = props.project.open.repo_root.clone();
    let read = ctx
      .future(
        (root, props.pipeline, props.latest, self.retry.get()),
        |(root, id, ..)| async move {
          background::run(move || VersionHistory::load_in(&root, id).map_err(|error| messages::store_error(&error)))
            .await?
        },
      )
      .state()
      .get();
    let body: Vec<Element> = match (read.data.filter(|_| read.error.is_none()), read.error) {
      (Some(history), _) => self.rows(ctx, &props, &history),
      (None, Some(error)) => vec![self.failed(ctx, &error)],
      (None, None) => vec![reading(ctx)],
    };
    let separator = separator(ctx);
    menu_panel("pipeline-hub-versions")
      .width(Dimension::Px(WIDTH))
      .child(heading(ctx, &props.name))
      .with_children(body)
      .child(separator)
      .child(foot(
        ctx,
        "pipeline-hub-versions-foot",
        &ctx.t("pipeline_hub.versions.foot"),
      ))
  }
}

impl Versions {
  fn rows(&self, ctx: &Ctx, props: &Props, history: &VersionHistory) -> Vec<Element> {
    if history.versions().is_empty() {
      return vec![
        Text::new(&ctx.t("pipeline_hub.versions.none"))
          .id("pipeline-hub-versions-none")
          .variant(TypographyStyle::Caption)
          .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Sm))
          .into(),
      ];
    }
    history
      .versions()
      .iter()
      .map(|version| {
        let held_by = history
          .use_of(version.number, props.tasks)
          .expect("a version of the history has a use");
        self.row(ctx, props, (version, held_by))
      })
      .collect()
  }

  /// A version's row: its number, *current* when it is, when it was published and what holds it; at its end a check
  /// for the current one, or *View ›* while the pointer is on it.
  fn row(&self, ctx: &Ctx, props: &Props, (version, held_by): (&PublishedVersion, Use)) -> Element {
    let number = version.number;
    let current = matches!(held_by, Use::Tasks(_));
    let id = format!("pipeline-hub-version-{}", number.get());
    let mut top = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&number.to_string())
          .id(format!("{id}-number"))
          .variant(TypographyStyle::Description)
          .nowrap(),
      );
    if current {
      top = top.child(
        Text::new(&ctx.t("pipeline_hub.versions.current"))
          .id(format!("{id}-current"))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      );
    }
    let meta = Text::new(&meta(ctx, props.now, (version.published_at, held_by)))
      .id(format!("{id}-meta"))
      .variant(TypographyStyle::Caption)
      .nowrap();
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Tight.role())
      .child(top)
      .child(meta);
    let hovered = self.hovered.get() == Some(number.get());
    let trailing: Option<Element> = if hovered {
      Some(view_more(ctx, &id))
    } else {
      current.then(|| {
        icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)
          .id(format!("{id}-check"))
          .into()
      })
    };
    let (open, viewing) = (props.open.clone(), props.viewing.clone());
    let shown = Viewed::of(version, held_by);
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    Press::run(move || {
      viewing.set(Some(shown.clone()));
      open.set(false);
    })
    .button()
    .id(id)
    .describe("version", number.to_string())
    .describe("current", if current { "true" } else { "false" })
    .width(Dimension::Pct(100.0))
    .rounded(RadiusSize::Sm)
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .on_mouse_enter(move || enter.set(Some(number.get())))
    .on_mouse_leave(move || {
      leave.update(|now| {
        if *now == Some(number.get()) {
          *now = None;
        }
      })
    })
    .child(text)
    .with_children(trailing)
    .into()
  }

  /// The versions could not be read: the reason, and *Try again*.
  fn failed(&self, ctx: &Ctx, error: &Message) -> Element {
    let retry = self.retry.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .child(field_error(
        "pipeline-hub-versions-error",
        &ctx.t_args("pipeline_hub.versions.failed", [("reason", &*message_text(ctx, error))]),
      ))
      .child(
        ActionButton::new(
          "pipeline-hub-versions-retry",
          ButtonKind::Secondary,
          ctx.t("common.tryAgain"),
        )
        .small()
        .on_click(move || retry.update(|tries| *tries += 1)),
      )
      .into()
  }
}

/// *Versions* and the pipeline's name at its end.
fn heading(ctx: &Ctx, name: &str) -> Element {
  Row::new()
    .id("pipeline-hub-versions-heading")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_hub.versions.title"))
        .id("pipeline-hub-versions-title")
        .variant(ExtraTypography::InspectorSection.role())
        .nowrap(),
    )
    .child(Row::new().flex(1.0))
    .child(
      Text::new(name)
        .id("pipeline-hub-versions-name")
        .variant(TypographyStyle::Caption)
        .nowrap()
        .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// *View ›*, at the end of the row under the pointer.
fn view_more(ctx: &Ctx, id: &str) -> Element {
  Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(
      Text::new(&ctx.t("pipeline_hub.versions.view"))
        .id(format!("{id}-view"))
        .variant(TypographyStyle::Description)
        .nowrap(),
    )
    .child(icon(Icon::ChevronRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// The bones of the rows while the versions are read.
fn reading(ctx: &mut Ctx) -> Element {
  let rows = skeleton::list(ctx, "pipeline-hub-versions-reading", &BONES);
  Skeleton::new("pipeline-hub-versions-loading", ctx.t("pipeline_hub.versions.reading")).show(ctx, rows)
}

/// *Published Oct 27 · used by 13 tasks*, *Published Oct 12 · 2 runs on it* or *Published Sep 30 · no runs on it*.
pub(super) fn meta(ctx: &Ctx, now: Timestamp, (published_at, held_by): (Timestamp, Use)) -> Arc<str> {
  let (key, count) = match held_by {
    Use::Tasks(0) => ("pipeline_hub.versions.metaNoTask", 0),
    Use::Tasks(1) => ("pipeline_hub.versions.metaOneTask", 1),
    Use::Tasks(count) => ("pipeline_hub.versions.metaTasks", count),
    Use::Runs(0) => ("pipeline_hub.versions.metaNoRun", 0),
    Use::Runs(1) => ("pipeline_hub.versions.metaOneRun", 1),
    Use::Runs(count) => ("pipeline_hub.versions.metaRuns", count),
  };
  ctx.t_args(
    key,
    [
      ("date", date(ctx, now, published_at).to_string()),
      ("count", count.to_string()),
    ],
  )
}
