//! A task's page while it is read (`02 Task · Summary` `d5E8Y`, `03 Task · Conversation` `WAW2N`, `04 Task · Changes`
//! `J9qwsb` on `Loading states · work screens` `xed4a`): the way back, the key, the title and the three tabs stay real
//! once the task's row is known, and are bones until it is; the status line, the primary action, *More actions* and
//! the Changes count are bones until the run is read; under the tabs, the open tab's own skeleton. *Reading the task…*
//! (`task.page.loading`) names it.

use std::{sync::Arc, time::Instant};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, project::TaskKey};

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList, TaskTab, TaskView},
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, TaskSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    icon::{Icon, icon},
    pressable::opening,
    skeleton::{self, Skeleton, bone, fading_up, round_bone},
    tabs::{PageTab, page_tabs},
  },
};

/// The title's bone while the task's row is not known, the primary action's and *More actions*'.
const TITLE: f32 = 296.0;
const PRIMARY: f32 = 128.0;
const MORE: f32 = 28.0;
/// A chat avatar (`qU0XW`).
const AVATAR: f32 = 28.0;

/// The task the page reads: its project, its key, whether it is kept *Not shared*, its title once its row is known,
/// and when the page began reading it: the project's tasks first, then the task, one reading whose skeleton's delay
/// runs once.
pub struct Reading<'a> {
  pub project: ProjectId,
  pub key: &'a TaskKey,
  pub kept: bool,
  pub title: Option<Arc<str>>,
  pub since: Instant,
}

/// The page of `task` on `view` while it is read.
pub fn page(ctx: &mut Ctx, task: &Reading<'_>, view: &TaskView) -> Element {
  let header = header(ctx, task, view);
  let skeleton = Skeleton::new("task-loading", ctx.t("task.page.loading")).since(task.since);
  let body = match view.tab() {
    TaskTab::Conversation => {
      let bones = conversation(ctx);
      skeleton.fills().show(ctx, bones)
    }
    TaskTab::Changes => {
      let bones = super::changes::reading::panes(ctx);
      skeleton.fills().show(ctx, bones)
    }
    TaskTab::Summary => {
      let bones = super::summary::reading::bones(ctx);
      let body = skeleton.show(ctx, bones);
      ScrollVertical::new(body).id("task-summary-scroll").flex(1.0).into()
    }
  };
  Column::new()
    .id("task-frame")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(body)
    .into()
}

/// The header: the way back, the title with the actions' bones, the status line's bones and the tabs.
fn header(ctx: &mut Ctx, task: &Reading<'_>, view: &TaskView) -> Element {
  let title: Element = match &task.title {
    Some(title) => Text::new(title)
      .variant(TypographyStyle::Heading)
      .nowrap()
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
      .into(),
    None => skeleton::line(ctx, TypographyStyle::Heading, TITLE).into(),
  };
  let title_row = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(Row::new().flex(1.0).min_width(Dimension::Px(0.0)).child(title))
    .child(skeleton::button(PRIMARY))
    .child(skeleton::button(MORE));
  let status = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(round_bone(IconSize::Base.size()))
    .child(skeleton::line(ctx, TypographyStyle::Description, 158.0))
    .child(skeleton::line(ctx, TypographyStyle::Description, 184.0));
  Column::new()
    .id("task-header")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(TaskSpacing::Header.role())
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role()),
    )
    .spacing(ExtraSpacing::Loose.role())
    .child(back(ctx, task))
    .child(title_row)
    .child(status)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().top(SpacingSize::Xs))
        .child(tabs(ctx, task, view)),
    )
    .into()
}

/// `← Tasks / KON-12`, the key a bone until the task's row is known.
fn back(ctx: &mut Ctx, task: &Reading<'_>) -> Element {
  let navigation = Navigation::of(ctx);
  let tasks = Route::Project(task.project, ProjectPage::Tasks(TaskList::default()));
  let mut crumbs = vec![Crumb::to(ctx.t("task.crumbs.tasks"), navigation.to(tasks.clone()))];
  let known = task.title.is_some();
  if known {
    crumbs.push(Crumb::here(task.key.to_string()).mono());
  }
  let arrow = opening(navigation.to(tasks))
    .id("task-back")
    .describe("a11y_name", ctx.t("task.crumbs.back"))
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted));
  Row::new()
    .id("task-back-row")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(arrow)
    .child(breadcrumb(ctx, "task-crumbs", CrumbSeparator::Slash, crumbs))
    .with_children((!known).then(|| skeleton::line(ctx, TypographyStyle::Mono, 46.0)))
    .into()
}

/// Summary, Conversation and Changes, its count a bone.
fn tabs(ctx: &mut Ctx, task: &Reading<'_>, view: &TaskView) -> Element {
  let navigation = Navigation::of(ctx);
  let chosen = view.tab();
  let tabs: Vec<PageTab> = [TaskTab::Summary, TaskTab::Conversation, TaskTab::Changes]
    .into_iter()
    .map(|tab| {
      let page = ProjectPage::task(task.key.clone(), TaskView::of(tab), task.kept);
      let shown = PageTab::to(
        format!("task-tab-{}", tab.name()),
        ctx.t(tab.title_key()),
        chosen == tab,
        navigation.to(Route::Project(task.project, page)),
      );
      if tab == TaskTab::Changes {
        shown.count_reading()
      } else {
        shown
      }
    })
    .collect();
  page_tabs(ctx, "task-tabs", tabs).into()
}

/// One entry of the transcript in bones.
#[derive(Clone, Copy)]
enum Entry {
  /// A step's divider: its name, then its line.
  Step(f32),
  /// An agent's message: its avatar, name and time, and its lines.
  Message(&'static [f32]),
  /// An event under a message: its mark and its words.
  Event(f32),
}

/// The transcript's entries, oldest first (`WAW2N`).
const TRANSCRIPT: [Entry; 12] = [
  Entry::Step(262.0),
  Entry::Message(&[892.0, 392.0]),
  Entry::Event(218.0),
  Entry::Event(304.0),
  Entry::Step(216.0),
  Entry::Message(&[240.0]),
  Entry::Message(&[828.0, 458.0]),
  Entry::Event(262.0),
  Entry::Event(358.0),
  Entry::Step(316.0),
  Entry::Message(&[696.0]),
  Entry::Message(&[980.0, 762.0, 414.0]),
];

/// The Conversation: the transcript's bones anchored to the composer, the older entries fading upward, and the
/// composer keeping its surface with who it talks to, its hint and Send as bones.
fn conversation(ctx: &Ctx) -> Column {
  let composer = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .rounded(RadiusSize::Lg)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .background(PaletteColor::SurfaceInput)
    .child(skeleton::line(ctx, TypographyStyle::Body, 116.0))
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(icon(Icon::Paperclip, IconSize::Md, PaletteColor::TextMuted))
        .child(bone(150.0, 22.0).rounded(ExtraRadius::Full.role()))
        .child(Spacer::new().flex(1.0))
        .child(skeleton::line(ctx, TypographyStyle::Description, 200.0))
        .child(skeleton::button(148.0)),
    );
  transcript(ctx).child(composer)
}

/// The transcript's bones over its composer: anchored to its foot, the older entries fading upward, as the chat is.
pub(super) fn transcript(ctx: &Ctx) -> Column {
  let entries = TRANSCRIPT.iter().map(|entry| entry_bones(ctx, *entry)).collect();
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(
      Padding::new()
        .top(SpacingSize::Lg)
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role())
        .bottom(SpacingSize::Lg),
    )
    .spacing(SpacingSize::Lg)
    .justify(lurq::layout::layout_kind::Justify::End)
    .with_children(fading_up(entries))
}

fn entry_bones(ctx: &Ctx, entry: Entry) -> Row {
  match entry {
    Entry::Step(name) => Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(skeleton::line(ctx, TypographyStyle::Description, name))
      .child(
        Row::new()
          .flex(1.0)
          .border_top(Border::inside(BorderSize::Sm, ExtraColor::Hover.role())),
      ),
    Entry::Message(lines) => Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .child(round_bone(AVATAR))
      .child(
        Column::new()
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .spacing(SpacingSize::Sm)
          .child(
            Row::new()
              .spacing(SpacingSize::Sm)
              .align_items(Alignment::Center)
              .child(skeleton::line(ctx, TypographyStyle::Body, 60.0))
              .child(skeleton::line(ctx, TypographyStyle::Caption, 32.0)),
          )
          .with_children(
            lines
              .iter()
              .map(|width| skeleton::line(ctx, TypographyStyle::Body, *width)),
          ),
      ),
    Entry::Event(words) => Row::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().left(SpacingSize::Xl))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(round_bone(12.0))
      .child(skeleton::line(ctx, TypographyStyle::Description, words)),
  }
}
