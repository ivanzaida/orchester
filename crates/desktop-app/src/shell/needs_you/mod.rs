//! What waits for the person in the projects the tabs show, and the title bar's *N need you* pill
//! (`Needs you` `O3eKK`, its states in `n6Nbi` of `eBhFg`). One thing waiting is what the project's
//! *Needs you* counts (`ProjectTasks::waiting`): a request for a decision, a question, a plan, a run that
//! waits after its retries, a run recorded interrupted. The pill counts every project the tabs show and its
//! tooltip splits the count by project; with nothing waiting it stays in place, quiet, at 0. A press opens the
//! Needs you panel under it ([`panel`]), and a press again, Esc or a press outside closes it; while it is
//! open the pill wears a `warning` stroke.

mod in_place;
mod item;
mod panel;
mod words;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{CursorIcon, Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, name::Name};
use orchester_workspace::{runner::state::RunnerState, tasks::data::ProjectTasks};

use self::panel::{NeedsYouPanel, PanelProps};
use crate::{
  chrome::PillRef,
  navigation::{Navigation, tabs::TabSet},
  project::{Projects, TasksState},
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// `Needs you` (`O3eKK`) height.
const PILL_HEIGHT: f32 = 26.0;
/// From the pill's bottom to the panel's top: the pill ends 31 into the title bar, the panel starts at 40
/// (`NSkXI` and `BrP6i` in `VnLOT`).
const PANEL_GAP: f32 = 9.0;

/// How many things wait for the person in one project the tabs show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
  pub project: ProjectId,
  pub name: Name,
  pub count: usize,
}

/// How many things wait for the person in `project`, once it is open and its tasks are read. Reading it
/// in a render re-renders it when that changes.
pub fn needs_you(projects: &Projects, project: ProjectId) -> Option<usize> {
  projects.state(project)?.open()?;
  projects
    .feed(project)
    .tasks
    .with(|tasks| tasks.ready().map(ProjectTasks::needs_you))
}

/// Why a tab of a project carries a mark (handover §3.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attention {
  /// This many things wait for the person.
  Waiting(usize),
  /// The project's runner is offline and does not start on its own.
  RunnerOffline,
  /// The project's tasks could not be read, or not read again.
  TasksUnread,
}

/// What a tab of `project` marks, if anything: what waits for the person first, then an unresolved
/// failure. Reading it in a render re-renders it when that changes.
pub fn attention(projects: &Projects, project: ProjectId) -> Option<Attention> {
  let open = projects.state(project)?.open()?.clone();
  if let Some(count) = needs_you(projects, project).filter(|count| *count > 0) {
    return Some(Attention::Waiting(count));
  }
  let feed = projects.feed(project);
  let offline = feed.status.with(|status| {
    status
      .as_ref()
      .is_some_and(|status| matches!(status.state_for(&open.repo_root), RunnerState::Failed { .. }))
  });
  if offline {
    return Some(Attention::RunnerOffline);
  }
  let unread = feed.stale.with(Option::is_some) || feed.tasks.with(|tasks| matches!(tasks, TasksState::Failed(_)));
  unread.then_some(Attention::TasksUnread)
}

/// Each project the tabs show whose tasks are read, in the order of the tabs, and what waits in it.
pub fn waiting(projects: &Projects, tabs: &TabSet) -> Vec<Waiting> {
  let mut all: Vec<Waiting> = Vec::new();
  for project in tabs.tabs().iter().filter_map(|tab| tab.route().project()) {
    if all.iter().any(|waiting| waiting.project == project) {
      continue;
    }
    let (Some(count), Some(name)) = (needs_you(projects, project), projects.name(project)) else {
      continue;
    };
    all.push(Waiting { project, name, count });
  }
  all
}

/// The words of `count` things waiting: *1 needs you*, *3 need you*.
pub fn count_words(ctx: &Ctx, count: usize) -> String {
  let key = if count == 1 {
    "shell.needsYou.one"
  } else {
    "shell.needsYou.other"
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}

/// The title bar's pill and the panel it opens. Nothing while no project the tabs show has its tasks read.
pub struct NeedsYouPill {
  open: Signal<bool>,
  /// The pill itself, which the panel opens under.
  anchor: ElementRef,
}

impl Component for NeedsYouPill {
  type Props = PillRef;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let tabs = navigation.tabs();
    let all = waiting(&Projects::of(ctx), &tabs);
    if all.is_empty() {
      return Element::new();
    }
    let total: usize = all.iter().map(|waiting| waiting.count).sum();
    let open = self.open.get();
    let pill = self.pill(ctx, (&all, total), open);
    // Mounted only while open, so what the panel expanded and kept goes when it closes.
    let panel = if open {
      ctx.mount_keyed::<NeedsYouPanel>("needs-you-panel", PanelProps(self.open.clone()))
    } else {
      Element::new()
    };
    let popup = Popup::new(self.anchor.clone(), panel)
      .open(self.open.clone())
      .placement(Placement::BottomEnd)
      .offset(0.0, PANEL_GAP);
    // `Trailing` (`AqLkT`): 8 between the pill and the window controls.
    Row::new()
      .ref_element(ctx.props::<PillRef>().0.clone())
      .padding(Padding::new().right(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .child(pill)
      .child(popup)
      .into()
  }
}

impl NeedsYouPill {
  /// The pill: amber while anything waits, a quiet outline at 0 (`UThn1`), a `warning` stroke while the panel
  /// is open (`z2PFA6`); its tooltip splits the count by project.
  fn pill(&self, ctx: &mut Ctx, (all, total): (&[Waiting], usize), open: bool) -> Element {
    let color = if total > 0 {
      PaletteColor::Warning
    } else {
      PaletteColor::TextMuted
    };
    let toggle = self.open.clone();
    let pill = pressable(move || toggle.update(|open| *open = !*open))
      .id("needs-you-pill")
      .ref_element(self.anchor.clone())
      .describe("count", total.to_string())
      .describe("open", open.to_string())
      .describe("a11y_name", count_words(ctx, total))
      .cursor(CursorIcon::Pointer)
      .height(Dimension::Px(PILL_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Hand, IconSize::Md, color.clone()))
      .child(
        Text::new(&count_words(ctx, total))
          .variant(TypographyStyle::Label)
          .color(color)
          .nowrap(),
      );
    let pill = if total > 0 {
      pill.background(PaletteColor::WarningMuted)
    } else {
      pill
    };
    let pill = match (open, total > 0) {
      (true, _) => pill.border_inside(BorderSize::Sm, PaletteColor::Warning),
      (false, false) => pill.border_inside(BorderSize::Sm, PaletteColor::Border),
      (false, true) => pill,
    };
    Tooltip::new("needs-you-pill", per_project(ctx, all)).wrap(ctx, pill)
  }
}

/// The tooltip's split by project: *Kontur 5 · Atlas 2*.
fn per_project(ctx: &Ctx, all: &[Waiting]) -> String {
  let separator = ctx.t("shell.needsYou.separator");
  all
    .iter()
    .map(|waiting| {
      ctx
        .t_args(
          "shell.needsYou.inProject",
          [
            ("project", waiting.name.as_str().to_owned()),
            ("count", waiting.count.to_string()),
          ],
        )
        .to_string()
    })
    .collect::<Vec<_>>()
    .join(&separator)
}
