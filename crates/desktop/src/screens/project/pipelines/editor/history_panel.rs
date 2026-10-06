//! Version history (`DZjhX` in 03.07 `fvPvA`, states `H0l9D`), in place of the inspector while a
//! published version is shown: the pipeline's published versions, newest first, each with when it
//! was published, how much changed from the version before and how many runs pinned it. The
//! selected version opens with its changes (or, compared, what the draft changes from it, 03.08
//! `Fb42P`), the runs that used it, and Compare and Restore as draft. Restoring replaces the
//! draft's steps with the version's; while the draft has changes of its own, the first click arms
//! the button, which turns primary and asks for a second click (03.09 `NmNRM`).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineChange, StepChange, VersionNumber, changes},
  time::Timestamp,
};

use super::EditorHandle;

/// The close button's size (`xTCNm`), and the slot that keeps a change's icon on the first line of
/// its text (`Q1Geah`).
const CLOSE_SIZE: f32 = 24.0;
const CHANGE_ICON_SLOT: f32 = 17.0;
use crate::{
  display,
  messages::Message,
  navigation::route::PipelineView,
  pipelines::{
    describe::{self, change_count, run_count},
    history::{PublishedVersion, VersionHistory},
  },
  screens::project::pipelines::PipelinesNav,
  theme::{ExtraSpacing, ExtraTypography, gutter_scrollbar},
  ui::{
    button::ActionButton,
    gutter::{beside_gutter, clear_of_gutter},
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
    pressable::{pressable, surface},
  },
};

/// The versions as far as they are read.
#[derive(Clone, Debug, PartialEq)]
pub enum HistoryState {
  Loading,
  Ready(Arc<VersionHistory>),
  Failed(Message),
}

/// What the editor holds, which the selected version is compared with and which restoring
/// replaces.
#[derive(Clone, Debug, PartialEq)]
pub struct Current {
  /// The number the draft gets when published.
  pub draft: u32,
  /// Whether the editor holds changes since the latest version.
  pub has_changes: bool,
  /// Its steps, when they are valid and can be compared.
  pub body: Option<Arc<Pipeline>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryPanelProps {
  pub pipeline: ResourceId,
  pub view: PipelineView,
  pub history: HistoryState,
  pub current: Current,
}

impl lurq::app::component::DevtoolsInspectable for HistoryPanelProps {}

/// The version Restore asked about: the next click on it replaces the draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asked(Option<VersionNumber>);

impl lurq::app::component::DevtoolsInspectable for Asked {}

pub struct HistoryPanel {
  handle: EditorHandle,
  nav: PipelinesNav,
  confirm: Signal<Asked>,
}

impl Component for HistoryPanel {
  type Props = HistoryPanelProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      handle: ctx
        .use_context::<EditorHandle>()
        .expect("the editor provides EditorHandle"),
      nav: ctx
        .use_context::<PipelinesNav>()
        .expect("the pipelines screen provides PipelinesNav"),
      confirm: ctx.signal(Asked(None)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HistoryPanelProps>().clone();
    // The versions scroll beside the gutter; what does not scroll keeps out of it.
    let body = match &props.history {
      HistoryState::Loading => {
        clear_of_gutter(Text::new(&ctx.t("pipelines.history.loading")).variant(TypographyStyle::Caption))
      }
      HistoryState::Failed(error) => clear_of_gutter(Notice::new(
        Icon::AlertCircle,
        ctx.t("pipelines.history.failed"),
        message_text(ctx, error),
      )),
      HistoryState::Ready(history) => self.versions(ctx, &props, history),
    };
    Column::new()
      .id("pipeline-history")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(SpacingSize::Lg)
      .padding_right(beside_gutter(ctx, SpacingSize::Lg))
      .spacing(SpacingSize::Md)
      .child(clear_of_gutter(self.header(ctx, props.pipeline)))
      .child(body)
  }
}

impl HistoryPanel {
  fn header(&self, ctx: &Ctx, pipeline: ResourceId) -> Element {
    let nav = self.nav.clone();
    let close = surface()
      .id("pipeline-history-close")
      .size(Dimension::Px(CLOSE_SIZE), Dimension::Px(CLOSE_SIZE))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(RadiusSize::Sm)
      .child(icon(Icon::X, IconSize::Dismiss, PaletteColor::TextSecondary));
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::History, IconSize::Option, PaletteColor::TextPrimary))
      .child(
        Text::new(&ctx.t("pipelines.editor.versionHistory"))
          .variant(TypographyStyle::Title)
          .flex(1.0),
      )
      .child(
        pressable(close, move || nav.show(pipeline, PipelineView::Draft))
          .hovered_style(Style::new().background(PaletteColor::AccentMuted)),
      )
      .into()
  }

  fn versions(&self, ctx: &Ctx, props: &HistoryPanelProps, history: &VersionHistory) -> Element {
    let now = Timestamp::now();
    let latest = history.latest().map(|latest| latest.number);
    let selected = props.view.version();
    let mut list = Column::new().width(Dimension::Pct(100.0)).spacing(SpacingSize::Sm);
    for version in history.versions() {
      list = list.child(self.row(ctx, props, version, latest == Some(version.number), now));
      if selected == Some(version.number) {
        list = list.child(self.details(ctx, props, version));
      }
    }
    ScrollVertical::new(list)
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .scrollbar(gutter_scrollbar())
      .into()
  }

  /// A version: its number, "Latest" on the newest, how long ago it was published, and how much
  /// changed and how many runs used it. Selecting it shows it on the canvas.
  fn row(
    &self,
    ctx: &Ctx,
    props: &HistoryPanelProps,
    version: &PublishedVersion,
    latest: bool,
    now: Timestamp,
  ) -> Element {
    let selected = props.view.version() == Some(version.number);
    let number = version.number.to_string();
    let mut title = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t_args("pipelines.history.version", [("version", number.as_str())]))
          .variant(TypographyStyle::Label),
      );
    if latest {
      title = title.child(Text::new(&ctx.t("pipelines.history.latest")).variant(ExtraTypography::MonoTag.role()));
    }
    let title = title.child(Spacer::new().flex(1.0)).child(
      Text::new(&message_text(ctx, &display::elapsed(now, version.published_at))).variant(TypographyStyle::Caption),
    );
    let changed = version.changes.as_ref().map_or_else(
      || ctx.t("pipelines.history.firstVersion"),
      |from| message_text(ctx, &change_count(from.changes.len())),
    );
    let summary = ctx.t_args(
      "pipelines.history.summary",
      [
        ("changes", &*changed),
        ("runs", &*message_text(ctx, &run_count(version.runs.len()))),
      ],
    );
    let content = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .child(title)
      .child(Text::new(&summary).variant(TypographyStyle::Caption));
    let row = surface()
      .id(format!("pipeline-version-{}", version.number.get()))
      .describe("selected", if selected { "true" } else { "false" })
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Stack.role())
      .rounded(RadiusSize::Md)
      .child(content);
    if selected {
      return row.background(PaletteColor::AccentMuted).into();
    }
    self
      .nav
      .link(props.pipeline, PipelineView::Version(version.number))
      .on(row)
      .hovered_style(Style::new().background(PaletteColor::AccentMuted))
      .into()
  }

  /// The selected version's changes or comparison, its runs, and Compare and Restore.
  fn details(&self, ctx: &Ctx, props: &HistoryPanelProps, version: &PublishedVersion) -> Element {
    let comparing = matches!(props.view, PipelineView::Compare(_));
    let changes = if comparing {
      comparison(ctx, version, &props.current)
    } else {
      version_changes(ctx, version)
    };
    let runs = if version.runs.is_empty() {
      ctx.t("pipelines.history.noRuns")
    } else {
      let numbers: Vec<String> = version.runs.iter().map(ToString::to_string).collect();
      numbers.join(&ctx.t("common.listSeparator")).into()
    };
    // The text lines up with the row's; the buttons span the panel as the inspector's do.
    let text = Column::new()
      .width(Dimension::Pct(100.0))
      .padding_horizontal(ExtraSpacing::Stack.role())
      .spacing(SpacingSize::Md)
      .child(changes)
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(ExtraSpacing::Tight.role())
          .child(Text::new(&ctx.t("pipelines.history.runs")).variant(TypographyStyle::Caption))
          .child(
            Text::new(&runs)
              .variant(TypographyStyle::FieldLabel)
              .color(PaletteColor::TextPrimary)
              .id("pipeline-version-runs"),
          ),
      );
    Column::new()
      .id("pipeline-version-details")
      .width(Dimension::Pct(100.0))
      .padding_bottom(SpacingSize::Sm)
      .spacing(SpacingSize::Md)
      .child(text)
      .child(self.actions(ctx, props, version, comparing))
      .into()
  }

  fn actions(&self, ctx: &Ctx, props: &HistoryPanelProps, version: &PublishedVersion, comparing: bool) -> Element {
    let (pipeline, number) = (props.pipeline, version.number);
    let compare_label = if comparing {
      ctx.t("pipelines.history.hideComparison")
    } else if props.current.has_changes {
      ctx.t_args(
        "pipelines.history.compareDraft",
        [("draft", props.current.draft.to_string().as_str())],
      )
    } else {
      ctx.t("pipelines.history.compareLatest")
    };
    // Comparing the steps the editor holds with themselves shows nothing.
    let same = props.current.body.as_deref() == Some(&*version.body);
    let view = if comparing {
      PipelineView::Version(number)
    } else {
      PipelineView::Compare(number)
    };
    let compare = ActionButton::new(compare_label)
      .icon(Icon::GitCompareArrows)
      .id("pipeline-history-compare")
      .fill_width()
      .link(self.nav.link(pipeline, view))
      .enabled(comparing || !same);
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(compare.render(ctx))
      .child(self.restore_button(ctx, props, version))
      .into()
  }

  /// Restore as draft. Unavailable when the editor already holds this version's steps; while
  /// the draft has changes of its own, the first click arms it, primary, and the second replaces
  /// them.
  fn restore_button(&self, ctx: &Ctx, props: &HistoryPanelProps, version: &PublishedVersion) -> Element {
    let number = version.number;
    let armed = self.confirm.get() == Asked(Some(number));
    let label = if armed {
      ctx.t_args(
        "pipelines.history.confirmRestore",
        [("draft", props.current.draft.to_string().as_str())],
      )
    } else {
      ctx.t_args("pipelines.history.restore", [("version", number.to_string().as_str())])
    };
    let unchanged = props.current.body.as_deref() == Some(&*version.body);
    let ask_first = props.current.has_changes && !armed;
    let (confirm, handle, nav, body, pipeline) = (
      self.confirm.clone(),
      self.handle.clone(),
      self.nav.clone(),
      version.body.clone(),
      props.pipeline,
    );
    let button = ActionButton::new(label)
      .icon(Icon::Undo2)
      .id("pipeline-history-restore")
      .fill_width();
    let button = if armed { button.primary() } else { button };
    let button = button
      .on_click(move || {
        if ask_first {
          confirm.set(Asked(Some(number)));
          return;
        }
        confirm.set(Asked(None));
        handle.restore(&body);
        nav.replace(pipeline, PipelineView::Draft);
      })
      .enabled(!unchanged)
      .render(ctx);
    if armed { button.class("primary") } else { button }
  }
}

/// What changed from the version before, or that it is the first (`z2hXA` in `H0l9D`).
fn version_changes(ctx: &Ctx, version: &PublishedVersion) -> Element {
  let Some(from) = &version.changes else {
    return Column::new()
      .id("pipeline-version-changes")
      .width(Dimension::Pct(100.0))
      .child(change_text(&ctx.t("pipelines.publish.firstVersion")))
      .into();
  };
  let heading = ctx.t_args(
    "pipelines.publish.changes",
    [("version", from.version.to_string().as_str())],
  );
  change_list(ctx, &heading, &from.changes, Some(ctx.t("pipelines.publish.noChanges")))
}

/// What the editor's steps change from `version`.
fn comparison(ctx: &Ctx, version: &PublishedVersion, current: &Current) -> Element {
  let number = version.number.to_string();
  let draft = current.draft.to_string();
  let heading = if current.has_changes {
    ctx.t_args(
      "pipelines.history.fromVersionToDraft",
      [("version", number.as_str()), ("draft", draft.as_str())],
    )
  } else {
    ctx.t_args("pipelines.history.fromVersionToLatest", [("version", number.as_str())])
  };
  let Some(body) = &current.body else {
    let explanation = ctx.t_args("pipelines.history.compareInvalid", [("draft", draft.as_str())]);
    return change_list(ctx, &heading, &[], Some(explanation));
  };
  let found = changes(&version.body, body);
  change_list(ctx, &heading, &found, Some(ctx.t("pipelines.history.noDifferences")))
}

/// A heading over one line per change (`MoO8N`); `empty` when there are none.
fn change_list(ctx: &Ctx, heading: &str, found: &[PipelineChange], empty: Option<Arc<str>>) -> Element {
  let mut list = Column::new()
    .id("pipeline-version-changes")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(Text::new(heading).variant(TypographyStyle::Caption));
  if found.is_empty()
    && let Some(empty) = empty
  {
    list = list.child(change_text(&empty));
  }
  for change in found {
    // A line that wraps keeps its icon on its first line (`DOroV`).
    let slot = Row::new()
      .height(Dimension::Px(CHANGE_ICON_SLOT))
      .align_items(Alignment::Center)
      .child(icon(change_icon(change), IconSize::Small, PaletteColor::TextSecondary));
    list = list.child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(slot)
        .child(change_text(&message_text(ctx, &describe::change(change))).flex(1.0)),
    );
  }
  list.into()
}

/// A change, or a line in its place (`o6to5`, `nRmqd`).
fn change_text(text: &str) -> Text {
  Text::new(text)
    .variant(ExtraTypography::ChangeLine.role())
    .width(Dimension::Pct(100.0))
}

fn change_icon(change: &PipelineChange) -> Icon {
  match change {
    PipelineChange::Step {
      change: StepChange::Added,
      ..
    } => Icon::Plus,
    PipelineChange::Step {
      change: StepChange::Removed,
      ..
    } => Icon::Minus,
    PipelineChange::Step { .. } | PipelineChange::Description => Icon::Pencil,
  }
}
