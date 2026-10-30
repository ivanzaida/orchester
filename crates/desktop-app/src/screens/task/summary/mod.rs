//! The Summary tab (ORC-100, `01 · Summary` `AsE7R`, the full page `01b` `HvPSr`, waiting for a decision `01c`
//! `SaKMC`): what the task asks for and where it stands, in one page that scrolls whole. In its column, 28 apart: the
//! decision the run waits for, as a card at the top ([`decisions`], `Decision cards` `CqVv3`), and returning the work
//! of a run that ended to a step; the run (`run`: its steps on a rail and its last event); the brief with the version
//! of the requirements it shows (`brief`) and the acceptance criteria; what the run made (`output`); and the task's
//! runs (`runs`). Beside it, 40 away, the task's properties with its run's facts and its memory (`properties`). Before
//! the task's first run the column says there are no runs yet (`empty`) over the brief; a task kept *Not shared*
//! says why it is read-only, or asks *Share task* in place; a task whose latest run cannot be read says why.

mod brief;
mod decisions;
mod empty;
mod live;
mod notice;
mod output;
mod properties;
pub(super) mod reading;
mod run;
mod run_account;
mod run_facts;
pub(super) mod runs;
mod steps;
mod version;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

pub use self::{
  decisions::{Waiting, decisions, returnable, waiting},
  notice::EventWords,
  version::Version,
};
use self::{runs::Runs, version::VersionMenu};
use super::{PageState, TaskContext, sharing};
use crate::{
  theme::{TaskSpacing, TaskText},
  ui::{
    icon::Icon,
    message::message_text,
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// The properties' column (`Properties` `Sq8ss`).
const PROPERTIES_WIDTH: f32 = 260.0;

#[derive(Clone, PartialEq)]
pub struct SummaryProps {
  pub task: TaskContext,
}

impl lurq::app::component::DevtoolsInspectable for SummaryProps {}

pub struct TaskSummary {
  /// The brief's version menu.
  versions: VersionMenu,
  /// The run's other facts are open under its group of the properties.
  more_facts: Signal<bool>,
}

impl Component for TaskSummary {
  type Props = SummaryProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      versions: VersionMenu::new(ctx),
      more_facts: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SummaryProps>().clone();
    let task = &props.task;
    let detail = &task.detail;
    let linked = sharing::linked(ctx, task);
    let next = sharing::next_key(ctx, task);
    let section = sharing::section(ctx, task, (linked.as_ref(), next.as_ref()));
    let state = PageState::of(ctx);
    let column: Vec<Element> = if detail.not_shared() {
      self.kept_parts(ctx, task, (linked.as_ref(), next.as_ref()))
    } else if detail.before_first_run() {
      std::iter::once(empty::empty(ctx, task))
        .chain(self.requirements(ctx, task))
        .collect()
    } else if let Some(unreadable) = &detail.latest_unreadable {
      let notice = Notice::new(
        "task-notice",
        NoticeTone::Danger,
        message_text(ctx, &unreadable.title()),
      )
      .icon(Icon::FileWarning)
      .body(message_text(ctx, &unreadable.detail));
      let actions = reason_line("task-unreadable-actions", &message_text(ctx, &unreadable.actions()));
      // What the task asks for can still be read, and its other runs.
      [notice.into(), actions.into()]
        .into_iter()
        .chain(self.requirements(ctx, task))
        .chain(runs_section(ctx, task))
        .collect()
    } else {
      self.parts(ctx, task)
    };
    let properties = properties::column(ctx, task, (section, &self.more_facts));
    layout(column, properties, state.summary_scroll)
  }
}

impl TaskSummary {
  /// The column's sections, in the design's order.
  fn parts(&self, ctx: &mut Ctx, task: &TaskContext) -> Vec<Element> {
    let detail = &task.detail;
    let decision = decisions(ctx, task);
    let ended = detail.run.as_ref().is_some_and(|run| run.run.status().is_terminal());
    let returning = ended.then(|| decisions::return_card(ctx, task)).flatten();
    // A run a teammate holds of a shared task is said even though this computer does not have it.
    let run = run::section(ctx, task, detail.run.as_ref());
    let output = detail.run.as_ref().map(|run| output::section(ctx, task, run));
    decision
      .into_iter()
      .chain(returning)
      .chain([run])
      .chain(self.requirements(ctx, task))
      .chain(output)
      .chain(runs_section(ctx, task))
      .collect()
  }

  /// A *Not shared* task's column (`R4elt`): why it is read-only, or *Share task*'s question in place (`TfVtK`), then
  /// what it asks for, its run and its runs.
  fn kept_parts(
    &self,
    ctx: &mut Ctx,
    task: &TaskContext,
    (linked, next): (Option<&sharing::Linked>, Option<&orchester_core::project::TaskKey>),
  ) -> Vec<Element> {
    let top = if PageState::of(ctx).share.get() {
      ctx.mount_keyed::<sharing::question::ShareQuestion>(
        "task-share-question-component",
        sharing::question::Props {
          task: task.clone(),
          linked: linked.cloned(),
          next: next.cloned(),
        },
      )
    } else {
      sharing::notice(ctx, task, linked)
    };
    let run = task.detail.run.as_ref().map(|run| run::section(ctx, task, Some(run)));
    std::iter::once(top)
      .chain(run)
      .chain(self.requirements(ctx, task))
      .chain(runs_section(ctx, task))
      .collect()
  }

  /// The brief, in the version the person picked, and its acceptance criteria.
  fn requirements(&self, ctx: &mut Ctx, task: &TaskContext) -> [Element; 2] {
    let state = PageState::of(ctx);
    let requirements = &task.detail.requirements;
    let chosen = state.requirements.get().map(|Version(source)| source);
    let shown = requirements.shown(chosen);
    // Set only when it changes: what is shown is then what the menu holds.
    if chosen != Some(shown) {
      state.requirements.set(Some(Version(shown)));
    }
    let details = requirements
      .details(shown)
      .expect("the version shown is one the task kept")
      .clone();
    let version = self.versions.chip(ctx, requirements, (shown, &state.requirements));
    let fallback = version::fallback(ctx, requirements, shown);
    [
      brief::brief(ctx, &details, (version, fallback)),
      brief::criteria(ctx, &details),
    ]
  }
}

/// *Runs* with every run of the task, as the column ends.
fn runs_section(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let removal = PageState::of(ctx).removal.get();
  runs::section(ctx, &Runs { task, removal })
}

/// A section's title with its count (`Heading · Acceptance criteria` `JYD1k`): 13/600, the count in 12 `text_muted`,
/// 8 apart.
pub fn section_head(ctx: &Ctx, id: &str, title: &str, count: Option<usize>) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(title)).variant(TaskText::SectionTitle.role()).nowrap())
    .with_children(count.map(|count| {
      Text::new(&count.to_string())
        .id(format!("{id}-count"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
    }))
}

/// The column of `parts` and the `properties` beside it, 24 from the top and 28 from the sides, scrolling together
/// when they are taller than the window (`Body` `PSNRw`).
fn layout(parts: Vec<Element>, properties: Element, scroll: ScrollState) -> Element {
  let column = Column::new()
    .id("task-summary-column")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(TaskSpacing::Sections.role())
    .with_children(parts);
  let properties = Column::new()
    .id("task-properties")
    .width(Dimension::Px(PROPERTIES_WIDTH))
    .flex_shrink(0.0)
    .padding(Padding::new().left(SpacingSize::Xl))
    .border_left(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(properties);
  let body = Row::new()
    .id("task-summary-body")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Xl)
        .left(TaskSpacing::Side.role())
        .right(TaskSpacing::Side.role())
        .bottom(TaskSpacing::Foot.role()),
    )
    .spacing(TaskSpacing::Columns.role())
    .align_items(Alignment::Start)
    .child(column)
    .child(properties);
  Column::new()
    .id("task-summary")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0))
    .child(
      ScrollVertical::new(body)
        .id("task-summary-scroll")
        .with_scroll_state(scroll)
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0)),
    )
    .into()
}
