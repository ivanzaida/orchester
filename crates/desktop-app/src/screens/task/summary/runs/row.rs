//! One run in *Runs* (`Run #8` `Km4KK` in `01b · Summary · full page`): 36 high, 10 at its sides, radius 6, the run the
//! page shows on `surface_raised`; its number in Geist Mono 11.5 `text_muted`, where it stands as its 13 glyph and
//! words in 12/500 in its tone, when it last changed and the tokens it used in 12 `text_muted`, what its worktree holds
//! (`line`), and its `…` (`Run menu` `EEk1r`): *Open its conversation*, *Open worktree in Files* and *Remove worktree*,
//! unavailable with why while the run works or has no worktree left. The removal's confirmation opens under the row;
//! while git removes, the row says so and the task's other rows wait; git's refusal stays under the row until the
//! person asks again.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{integration::Leftovers, project::TaskKey, run::RunNumber, tokens::compact_tokens};
use orchester_workspace::{
  files::Version,
  messages::Message,
  tasks::{
    history::RunRecord,
    integration_jobs::{JobState, RunJobState},
  },
};

use super::line::{Line, day_and_clock, line};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{FilesView, ProjectPage, Route, TaskView},
  screens::task::{PageState, RemovalPlace},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    message::message_text,
    status::Status,
    tooltip::Tooltip,
  },
};

/// A row's height (`Km4KK`) and its menu's width.
const ROW_HEIGHT: f32 = 36.0;
const MENU_WIDTH: f32 = 240.0;

#[derive(Clone, PartialEq)]
pub struct RowProps {
  pub project: ShownProject,
  pub key: TaskKey,
  /// The task is one kept *Not shared*: its page's links keep its own address.
  pub kept: bool,
  pub record: RunRecord,
  /// What the run left as last read, for its worktree; `None` for a run with live work.
  pub left: Option<Result<Leftovers, Message>>,
  /// What the run left as read now, while it has worktrees and no live work: it can be removed.
  pub removable: Option<Leftovers>,
  pub this_run: bool,
  pub job: RunJobState,
  /// Another run of the task is being removed or merged.
  pub others_busy: bool,
  /// Its removal's confirmation is open, in the row or under the header.
  pub confirming: bool,
  /// The run is recorded as running, but no runner looks after it.
  pub lost: bool,
  /// The step whose agent waits for the person's approval, by name.
  pub waits_for_approval: Option<Arc<str>>,
}

impl lurq::app::component::DevtoolsInspectable for RowProps {}

pub struct RunRow {
  /// The row's menu is open.
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for RunRow {
  type Props = RowProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let record = &props.record;
    let status = record.phase.map_or(Status::Unreadable, Status::of_run);
    let failed = match &props.job.remove {
      JobState::Failed(error) => Some(error.clone()),
      _ => None,
    };
    let number = record.number.map_or_else(
      || ctx.t("summary.runs.runUnnumbered"),
      |number| ctx.t_args("summary.runs.number", [("run", number.get().to_string())]),
    );
    let detail = match (&props.waits_for_approval, props.lost) {
      (Some(step), _) => ctx.t_args("summary.runs.waitsForApproval", [("step", &**step)]),
      (None, true) => ctx.t("summary.runs.lost"),
      (None, false) => message_text(ctx, &record.detail),
    };
    // The status's word, then where the run stands (`State` in `Km4KK`).
    let word = ctx.t(status.key());
    let stands = if detail.is_empty() {
      word
    } else {
      ctx.t_args("summary.runs.stands", [("status", word), ("detail", detail)])
    };
    let line = line(ctx, record, props.left.as_ref());
    let main = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ROW_HEIGHT))
      .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .child(
        Text::new(&number)
          .id(format!("run-row-{}-number", record.id))
          .variant(ExtraTypography::MonoKey.role())
          .nowrap(),
      )
      .child(icon(status.icon(), IconSize::Base, status.color()))
      .child(
        Text::new(&stands)
          .id(format!("run-row-{}-what", record.id))
          .variant(TypographyStyle::Label)
          .color(status.color())
          .nowrap(),
      )
      .child(
        Text::new(&when(ctx, record))
          .id(format!("run-row-{}-when", record.id))
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextMuted)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap(),
      )
      .with_children(line.as_ref().map(|line| worktree(ctx, record, line)))
      .child(self.menu(ctx, &props, line.as_ref()));
    let main = if props.this_run {
      main.background(PaletteColor::SurfaceRaised)
    } else {
      main
    };
    Column::new()
      .id(format!("run-row-{}", record.id))
      .describe("run", record.number.map_or(0, RunNumber::get).to_string())
      .width(Dimension::Pct(100.0))
      .child(main)
      .with_children(failed.as_ref().filter(|_| !props.confirming).map(|error| {
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
          .child(
            Text::new(&message_text(ctx, error))
              .id(format!("run-remove-error-{}", record.id))
              .variant(ExtraTypography::Note.role())
              .color(PaletteColor::Danger)
              .flex(1.0),
          )
      }))
  }
}

/// "15:04 · 412k tokens": when the run last changed, and the tokens it used.
fn when(ctx: &Ctx, record: &RunRecord) -> Arc<str> {
  let when = record
    .at
    .map_or_else(|| ctx.t("summary.runs.noTime"), |at| day_and_clock(ctx, at));
  match &record.run {
    Some(run) => ctx.t_args(
      "summary.runs.whenTokens",
      [("when", when.to_string()), ("tokens", compact_tokens(run.used().get()))],
    ),
    None => when,
  }
}

/// What the run's worktree holds in a word or two, in full in its tooltip.
fn worktree(ctx: &mut Ctx, record: &RunRecord, line: &Line) -> Element {
  let id = format!("run-row-{}-worktree", record.id);
  let shown = Row::new()
    .id(id.clone())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(line.glyph, IconSize::Sm, line.color.clone()))
    .child(
      Text::new(&ctx.t(line.short))
        .variant(ExtraTypography::Note.role())
        .color(line.color.clone())
        .nowrap(),
    );
  Tooltip::new(format!("{id}-tip"), line.text.clone()).wrap(ctx, shown)
}

impl RunRow {
  /// The row's `…`, with its menu while it is open.
  fn menu(&self, ctx: &mut Ctx, props: &RowProps, line: Option<&Line>) -> Element {
    let record = &props.record;
    let number = record.number.map_or(0, RunNumber::get);
    let open = self.open.clone();
    let label = ctx.t("summary.runs.menu.label");
    let button = IconButton::new(format!("run-menu-{number}"), Icon::Ellipsis, label)
      .small()
      .element(self.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
    let shown = self.open.get();
    let button = if shown { button.active() } else { button }.render(ctx);
    let menu = shown.then(|| {
      let items = vec![
        self.conversation(ctx, props),
        self.files(ctx, props, line),
        self.remove(ctx, props),
      ];
      let panel = menu_panel(&format!("run-menu-{number}-panel"))
        .width(Dimension::Px(MENU_WIDTH))
        .with_children(items);
      menu_at_end(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new().flex_shrink(0.0).child(button).with_children(menu).into()
  }

  /// *Open its conversation*: the task's Conversation, which holds every run's.
  fn conversation(&self, ctx: &mut Ctx, props: &RowProps) -> Element {
    let navigation = Navigation::of(ctx);
    let page = ProjectPage::task(props.key.clone(), TaskView::Conversation, props.kept);
    let route = Route::Project(props.project.open.project.id(), page);
    let number = props.record.number.map_or(0, RunNumber::get);
    MenuItem::opens(
      format!("run-open-conversation-{number}"),
      ctx.t("summary.runs.menu.conversation"),
      navigation.to(route),
    )
    .icon(Icon::MessageSquare)
    .render(ctx, &self.open)
  }

  /// *Open worktree in Files*: the code viewer at the run's worktree, while it is there.
  fn files(&self, ctx: &mut Ctx, props: &RowProps, line: Option<&Line>) -> Element {
    let number = props.record.number.map_or(0, RunNumber::get);
    let (id, label) = (format!("run-open-worktree-{number}"), ctx.t("task.more.openWorktree"));
    if !line.is_some_and(|line| line.present) {
      return MenuItem::unavailable(id, label, ctx.t("summary.runs.menu.noWorktree"))
        .icon(Icon::FolderTree)
        .render(ctx, &self.open);
    }
    let navigation = Navigation::of(ctx);
    let files = FilesView::new(None, Version::Run(props.record.id));
    let route = Route::Project(props.project.open.project.id(), ProjectPage::Files(files));
    MenuItem::opens(id, label, navigation.to(route))
      .icon(Icon::FolderTree)
      .render(ctx, &self.open)
  }

  /// *Remove worktree*, while the run has worktrees and no live work: it opens the confirmation under the row.
  fn remove(&self, ctx: &mut Ctx, props: &RowProps) -> Element {
    let number = props.record.number.map_or(0, RunNumber::get);
    let (id, label) = (format!("run-remove-worktree-{number}"), ctx.t("task.removeWorktree"));
    let unavailable = |reason: Arc<str>| MenuItem::unavailable(id.clone(), label.clone(), reason).icon(Icon::FolderX);
    let item = if props.record.without_live_work().is_none() && props.record.run.is_some() {
      unavailable(ctx.t("summary.runs.menu.removeWorking"))
    } else if props.removable.is_none() {
      unavailable(ctx.t("summary.runs.menu.noWorktree"))
    } else if props.job.removing() {
      unavailable(ctx.t("summary.runs.menu.removing")).without_lock()
    } else if props.confirming || props.job.busy() || props.others_busy {
      unavailable(ctx.t("task.more.waits")).without_lock()
    } else {
      let (state, run) = (PageState::of(ctx), props.record.id);
      let jobs = props.project.feed.jobs.of(run);
      MenuItem::new(id.clone(), label.clone(), move || {
        jobs.clear_failed_removal();
        state.open_removal(run, RemovalPlace::Row);
      })
      .icon(Icon::FolderX)
    };
    item.render(ctx, &self.open)
  }
}
