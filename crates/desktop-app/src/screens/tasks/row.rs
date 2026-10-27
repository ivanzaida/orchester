//! One line of the list (`KON-12` `r0eIJ` in `Tasks · redesign` `Lw1Gb`): 40 high, 8 at its sides, its cells 12 apart,
//! radius 6, the `hover` fill under the pointer. The status icon in its tone, the key (mono 11.5 `text_muted`, 52
//! wide), the title in 13/500 with its tags as plain words in 11.5 `text_muted`, the one state phrase in its tone, the
//! pipeline (12 `text_muted`, 150 wide, at its end), the priority's icon and the task's age (mono 11 `text_muted`).
//! Under the pointer the pipeline gives way to the row's actions: a task something of waits for shows its one action
//! first (Review, Answer, Review plan…, opening where it is decided), then every task but a *Not shared* one its *More
//! actions* menu (`task_close`). A press elsewhere opens the task on its Details. The task a Create just added is
//! tinted and carries the New badge. A group's header line is drawn here too ([`super::list::group_line`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  project::TaskKey,
  task::estimate::{EstimateSource, StoryEstimate},
};
use orchester_workspace::tasks::list::{PipelineLabel, TaskRow};

use super::{
  list::{GroupLine, group_line},
  phrase::Phrase,
};
use crate::{
  navigation::{Navigation, waiting::Destination},
  routes::{ProjectPage, Route, TaskView},
  screens::task_close::{self, Closer, MenuPlace, Subject},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, TasksSpacing},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, TagChip, badge},
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    status::priority_look,
    tooltip::Truncated,
  },
};

/// The row's height and its cells' widths (`r0eIJ`): key 52, the pipeline or the actions 150, priority 16, age 30;
/// the state phrase is as wide as its words up to 320, so the title keeps its room.
pub(super) const ROW_HEIGHT: f32 = 40.0;
const KEY_WIDTH: f32 = 52.0;
const TRAILING_WIDTH: f32 = 150.0;
const PRIORITY_WIDTH: f32 = 16.0;
const AGE_WIDTH: f32 = 30.0;
const STATE_MAX_WIDTH: f32 = 320.0;
/// The tags a row names; the rest are counted.
const SHOWN_TAGS: usize = 2;

/// What a task's row shows.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct RowProps {
  pub project: ProjectId,
  pub row: TaskRow,
  pub phrase: Phrase,
  /// What the person does for a task that waits for them.
  pub action: Option<NeedAction>,
  /// How long ago the task was created, in its largest whole unit (`2h`).
  pub age: Arc<str>,
  /// The task a Create just added.
  pub highlighted: bool,
  /// The project is shared: its team server keeps its tasks.
  pub shared: bool,
}

/// The one action of a task that waits for the person: its words and glyph, and where it opens or why it opens
/// nowhere.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct NeedAction {
  pub label: Arc<str>,
  pub glyph: Icon,
  pub destination: Result<Destination, Arc<str>>,
}

/// One item of the list: a group's header line or a task's row.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum ItemProps {
  Group(GroupLine),
  Task(Box<RowProps>),
}

impl DevtoolsInspectable for ItemProps {}

pub(super) struct ListItem {
  hovered: Signal<bool>,
  /// Where its *More actions* menu is open.
  menu: MenuPlace,
}

impl Component for ListItem {
  type Props = ItemProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
      menu: MenuPlace::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let line = match ctx.props::<ItemProps>().clone() {
      ItemProps::Group(line) => group_line(ctx, &line),
      ItemProps::Task(props) => self.task(ctx, &props),
    };
    // 2 under each line, as the list's lines are apart (`ZCJb1`), and the page's 28 at their end: the list runs to the
    // panel's edge for its scrollbar.
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .bottom(ExtraSpacing::Tight.role())
          .right(TasksSpacing::Sides.role()),
      )
      .child(line)
  }
}

impl ListItem {
  fn task(&self, ctx: &mut Ctx, props: &RowProps) -> Element {
    let row = &props.row;
    let key = &row.key;
    let task: TaskKey = key.parse().expect("a task's key reads back as a task key");
    // A task kept Not shared has an address of its own: a shared task may have its key (`R4elt`).
    let page = if row.not_shared {
      ProjectPage::KeptTask(task, TaskView::default())
    } else {
      ProjectPage::Task(task, TaskView::default())
    };
    let navigation = Navigation::of(ctx);
    let acting = (self.hovered.get() || self.menu.is_open()) && !row.not_shared;
    let trailing = if acting {
      actions(ctx, props, &self.menu)
    } else {
      pipeline(ctx, row)
    };
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .child(icon(props.phrase.glyph, IconSize::Md, props.phrase.glyph_color.clone()).id(format!("task-status-{key}")))
      .child(
        Row::new().width(Dimension::Px(KEY_WIDTH)).flex_shrink(0.0).child(
          Text::new(key)
            .id(format!("task-key-{key}"))
            .variant(ExtraTypography::MonoKey.role())
            .nowrap(),
        ),
      )
      .child(title_cell(ctx, row, props.highlighted))
      .child(state_cell(ctx, key, &props.phrase))
      .child(cell(TRAILING_WIDTH, Justify::End).child(trailing))
      .child(priority_cell(ctx, row))
      .child(
        cell(AGE_WIDTH, Justify::End).child(
          Text::new(&props.age)
            .id(format!("task-age-{key}"))
            .variant(ExtraTypography::MonoNote.role())
            .nowrap(),
        ),
      );
    let line = if props.highlighted {
      line
        .background(PaletteColor::InfoMuted)
        .class("highlighted")
        .describe("highlighted", "true")
    } else {
      line
    };
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    Press::Open(navigation.to(Route::Project(props.project, page)))
      .button()
      .id(format!("task-row-{key}"))
      .width(Dimension::Pct(100.0))
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .child(line)
      .into()
  }
}

/// A cell `width` wide, its content placed by `justify`.
fn cell(width: f32, justify: Justify) -> Row {
  Row::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .min_width(Dimension::Px(0.0))
    .justify(justify)
    .align_items(Alignment::Center)
}

/// The title, cut to its room, then *was KON-12* for a task that joined with another key (`Xr4Wb`), its story points
/// (ORC-49), its first tags as plain words, how many more there are, and the New badge.
fn title_cell(ctx: &mut Ctx, row: &TaskRow, highlighted: bool) -> Row {
  let key = &row.key;
  let note = |text: &str, id: String| {
    Text::new(text)
      .id(id)
      .variant(ExtraTypography::Note.role())
      .nowrap()
      .flex_shrink(0.0)
  };
  let points = row
    .estimate
    .as_ref()
    .map(|estimate| TagChip::new(format!("task-points-{key}"), points_chip(ctx, estimate)).mono());
  let tags: Vec<Text> = row
    .tags
    .iter()
    .take(SHOWN_TAGS)
    .enumerate()
    .map(|(index, name)| note(name, format!("task-tag-{key}-{index}")))
    .collect();
  let more = row.tags.len().saturating_sub(SHOWN_TAGS);
  let more = (more > 0).then(|| {
    let count = ctx.t_args("tasks.moreTags", [("count", more.to_string())]);
    note(&count, format!("task-more-tags-{key}"))
  });
  let new = highlighted.then(|| badge(&format!("task-new-{key}"), BadgeTone::Info, &ctx.t("tasks.newBadge")));
  Row::new()
    .id(format!("task-title-{key}"))
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Truncated::new(
        format!("task-title-text-{key}"),
        row.title.clone(),
        ExtraTypography::Strong.role(),
      )
      .render(ctx),
    )
    .with_children(row.former_key.as_ref().map(|former| {
      Text::new(&ctx.t_args("tasks.formerKey", [("key", &**former)]))
        .id(format!("task-former-key-{key}"))
        .variant(ExtraTypography::MonoKey.role())
        .nowrap()
        .flex_shrink(0.0)
    }))
    .with_children(points)
    .with_children(tags)
    .with_children(more)
    .with_children(new)
}

/// A task's story points as a chip says them: a person's as they are (`5 pt`), Orchester's estimate marked as one
/// (`~5 pt`).
pub(super) fn points_chip(ctx: &Ctx, estimate: &StoryEstimate) -> Arc<str> {
  let key = match estimate.source() {
    EstimateSource::Person => "tasks.points.person",
    EstimateSource::Automation => "tasks.points.estimated",
  };
  ctx.t_args(key, [("count", estimate.points().get().to_string())])
}

/// The one state phrase, in its tone, cut to its room.
fn state_cell(ctx: &mut Ctx, key: &str, phrase: &Phrase) -> Row {
  Row::new()
    .max_width(Dimension::Px(STATE_MAX_WIDTH))
    .min_width(Dimension::Px(0.0))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .child(
      Truncated::new(
        format!("task-state-{key}"),
        phrase.words.clone(),
        TypographyStyle::Description,
      )
      .color(phrase.color.clone())
      .render(ctx),
    )
}

/// The pipeline a task runs, cut to its cell.
fn pipeline(ctx: &mut Ctx, row: &TaskRow) -> Element {
  let label = pipeline_label(ctx, &row.pipeline);
  Truncated::new(
    format!("task-pipeline-{}", row.key),
    label,
    TypographyStyle::Description,
  )
  .color(PaletteColor::TextMuted)
  .render(ctx)
}

/// The priority's icon alone; its word names it for tools.
fn priority_cell(ctx: &Ctx, row: &TaskRow) -> Row {
  let (glyph, word, color) = priority_look(row.organization.priority());
  cell(PRIORITY_WIDTH, Justify::Center)
    .id(format!("task-priority-{}", row.key))
    .describe("a11y_name", ctx.t(word))
    .child(icon(glyph, IconSize::Base, color))
}

/// The row's actions under the pointer: a waiting task's one action, then the *More actions* menu.
fn actions(ctx: &mut Ctx, props: &RowProps, menu: &MenuPlace) -> Element {
  let row = &props.row;
  let key = &row.key;
  let primary = props.action.clone().map(|action| {
    let button = ActionButton::new(format!("task-action-{key}"), ButtonKind::Secondary, action.label)
      .icon(action.glyph)
      .small();
    match action.destination {
      Ok(destination) => {
        let navigation = Navigation::of(ctx);
        button.opens(Arc::new(move |at| destination.open(&navigation, at)))
      }
      Err(reason) => button.unavailable(reason),
    }
  });
  let subject = Subject {
    id: row.id,
    key: key.clone(),
    status: row.organization.status(),
    shared: props.shared,
  };
  let closer = Closer::of(ctx);
  let more = task_close::more_menu(ctx, (&closer, menu), &subject, &format!("task-more-{key}"));
  Row::new()
    .id(format!("task-actions-{key}"))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .with_children(primary)
    .child(more)
    .into()
}

/// The pipeline a task runs: a published one by its name and version as this list words it (`Feature delivery v3`),
/// otherwise in the workspace's words.
fn pipeline_label(ctx: &Ctx, pipeline: &PipelineLabel) -> Arc<str> {
  match pipeline {
    PipelineLabel::Published { name, version } => ctx.t_args(
      "tasks.pipelineVersion",
      [("name", name.to_string()), ("version", version.get().to_string())],
    ),
    other => message_text(ctx, &other.message()),
  }
}
