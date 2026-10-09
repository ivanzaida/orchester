//! Several operations under way (`Activity · operations` `JhuiW` of `haO09`): *3 operations · Publishing
//! Feature delivery v4…* on a raised fill, which opens the *Background operations* popover over it
//! (`pDdzQ`): the operations under way with where they are and how long they have run, *RECENT*
//! finished ones with their time, and *Open runner log*. Esc, a press outside or its close button closes
//! it.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::ShrinkLimit},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  activity::{ActivityLog, Event, Operation, Outcome, Place, Target},
  display,
};

use super::{
  activity::{event_text, task_key},
  facts::{BarAction, Dispatch, ProjectFacts},
  item::{cut, give_way, glyph, label},
};
use crate::{
  routes::Screen,
  shell::clock::{Clock, clock},
  theme::{ExtraColor, ExtraSpacing, IconSize, ShellText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
    popover::{PopoverSide, popover},
    pressable::pressable,
  },
};

/// `Activity · operations` height.
const ITEM_HEIGHT: f32 = 22.0;
/// `Background operations` width.
const POPOVER_WIDTH: f32 = 420.0;
/// The finished operations *RECENT* lists at most.
const RECENT_LIMIT: usize = 8;

#[derive(Clone)]
pub struct OperationsProps {
  pub log: ActivityLog,
  pub project: Option<ProjectFacts>,
  pub dispatch: Dispatch,
}

impl PartialEq for OperationsProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.log, &self.project) == (&other.log, &other.project)
  }
}

impl DevtoolsInspectable for OperationsProps {}

pub struct Operations {
  anchor: ElementRef,
  open: Signal<bool>,
}

impl Component for Operations {
  type Props = OperationsProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ctx.element_ref(),
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<OperationsProps>().clone();
    let running: Vec<&Operation> = props.log.running().collect();
    let Some(newest) = running.first() else {
      return Element::new();
    };
    let title = crate::ui::message::message_text(ctx, newest.title());
    let toggle = self.open.clone();
    let open = self.open.get();
    let item = pressable(move || toggle.update(|open| *open = !*open))
      .id("status-activity")
      .ref_element(self.anchor.clone())
      .describe("state", "several")
      .describe("running", running.len().to_string())
      .flex_shrink(1.0)
      .shrink_order(give_way::ACTIVITY)
      .shrink_limit(ShrinkLimit::Content)
      .height(Dimension::Px(ITEM_HEIGHT))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()))
      .child(glyph(Icon::LoaderCircle, PaletteColor::TextSecondary))
      .child(
        Text::new(&ctx.t_args("shell.operations.count", [("count", running.len().to_string())]))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      )
      .child(label(&ctx.t("shell.operations.separator"), PaletteColor::TextMuted))
      .child(cut(
        ctx,
        "status-activity-text",
        &ctx.t_args("shell.status.activity.running", [("title", title.to_string())]),
        PaletteColor::TextSecondary,
      ))
      .child(glyph(
        if open { Icon::ChevronUp } else { Icon::ChevronDown },
        PaletteColor::TextMuted,
      ));
    let mut row = Row::new()
      .flex_shrink(1.0)
      .shrink_order(give_way::ACTIVITY)
      .shrink_limit(ShrinkLimit::Content)
      .child(item);
    if open {
      let panel = panel(ctx, &props, &running, &self.open);
      row = row.child(popover(
        self.anchor.clone(),
        self.open.clone(),
        PopoverSide::Above,
        panel,
      ));
    }
    row.into()
  }
}

/// The popover's panel (`Background operations` `pDdzQ`).
fn panel(ctx: &mut Ctx, props: &OperationsProps, running: &[&Operation], open: &Signal<bool>) -> Element {
  let project = props.project.as_ref();
  let close = open.clone();
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Lg)
        .right(SpacingSize::Sm)
        .top(ExtraSpacing::Loose.role())
        .bottom(ExtraSpacing::Loose.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t("shell.operations.title")).variant(TypographyStyle::Label))
    .child(
      Text::new(&ctx.t_args("shell.operations.running", [("count", running.len().to_string())]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted),
    )
    .child(Row::new().flex(1.0))
    .child(
      IconButton::new("operations-close", Icon::X, ctx.t("shell.operations.close"))
        .on_click(move || close.set(false))
        .render(ctx),
    );
  let mut list = Column::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(ExtraSpacing::Tight.role())
    .child(section(&ctx.t("shell.operations.runningSection")));
  for (index, operation) in running.iter().enumerate() {
    let where_ = place_words(ctx, operation.project(), operation.target(), project);
    let time = clock(
      ctx,
      &format!("operation-{index}-time"),
      Clock::Since(operation.started()),
    );
    let title = ctx.t_args(
      "shell.status.activity.running",
      [(
        "title",
        crate::ui::message::message_text(ctx, operation.title()).to_string(),
      )],
    );
    list = list.child(row(
      &format!("operation-{index}"),
      (Icon::LoaderCircle, PaletteColor::TextSecondary),
      (&title, where_.as_deref()),
      time,
    ));
  }
  let recent: Vec<&Event> = props.log.recent().into_iter().take(RECENT_LIMIT).collect();
  if !recent.is_empty() {
    list = list
      .child(Row::new().padding(Padding::new().top(ExtraSpacing::Snug.role())))
      .child(section(&ctx.t("shell.operations.recent")));
    for (index, event) in recent.iter().enumerate() {
      let where_ = place_words(ctx, event.project(), event.target(), project);
      let time = label(&display::clock_time(event.at()), PaletteColor::TextMuted).into();
      list = list.child(row(
        &format!("operation-recent-{index}"),
        outcome_icon(event.outcome()),
        (&event_text(ctx, event, project), where_.as_deref()),
        time,
      ));
    }
  }
  let mut panel = Column::new()
    .id("operations-popover")
    .width(Dimension::Px(POPOVER_WIDTH))
    .rounded(RadiusSize::Lg)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::BorderStrong)
    .box_shadow(ShadowStyle::Lg)
    .clip()
    .child(head)
    .child(divider(ctx))
    .child(list);
  panel = panel.child(footer(ctx, props));
  panel.into()
}

/// *Open runner log* and that Esc closes the popover (`Footer` `Vn8Va`).
fn footer(ctx: &Ctx, props: &OperationsProps) -> Row {
  let dispatch = props.dispatch.clone();
  let log = props.project.as_ref().map(|_| {
    ActionButton::new(
      "operations-open-log",
      ButtonKind::Ghost,
      ctx.t("shell.status.runner.openLog"),
    )
    .icon(Icon::FileText)
    .on_click(move || dispatch(BarAction::OpenRunnerLog))
  });
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Sm)
        .right(SpacingSize::Md)
        .top(ExtraSpacing::Snug.role())
        .bottom(ExtraSpacing::Snug.role()),
    )
    .border_top(lurq::node::border::Border::inside(BorderSize::Sm, PaletteColor::Border))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(log)
    .child(Row::new().flex(1.0))
    .child(Text::new(&ctx.t("shell.operations.escCloses")).variant(TypographyStyle::Caption))
}

fn divider(ctx: &Ctx) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
    .background(PaletteColor::Border)
}

/// *RUNNING*, *RECENT*.
fn section(text: &str) -> Text {
  Text::new(text).variant(ShellText::MenuSection.role()).nowrap()
}

/// An operation's row: its icon, its title and where it is, and its time.
fn row(
  id: &str,
  (glyph_icon, color): (Icon, PaletteColor),
  (title, where_): (&str, Option<&str>),
  time: Element,
) -> Element {
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Hair.role())
    .child(
      Text::new(title)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextPrimary),
    )
    .with_children(where_.map(|where_| Text::new(where_).variant(TypographyStyle::Caption)));
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Xs, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(glyph_icon, IconSize::Base, color))
    .child(text)
    .child(time)
    .into()
}

const fn outcome_icon(outcome: &Outcome) -> (Icon, PaletteColor) {
  match outcome {
    Outcome::Finished => (Icon::CircleCheck, PaletteColor::Success),
    Outcome::Failed { .. } => (Icon::CircleX, PaletteColor::Danger),
    Outcome::NotDone => (Icon::CircleAlert, PaletteColor::TextMuted),
    Outcome::NeedsYou { .. } => (Icon::Hand, PaletteColor::Warning),
  }
}

/// Where an operation is: *Kontur · Pipelines*, *Kontur · KON-29*; its project alone where its place has
/// no name here.
fn place_words(ctx: &Ctx, project_name: Option<&str>, target: &Target, shown: Option<&ProjectFacts>) -> Option<String> {
  let name = project_name?;
  let Target::Project { place, repo_root } = target else {
    return Some(name.to_owned());
  };
  let here = shown.filter(|shown| shown.repo_root == *repo_root);
  let place = match place {
    Place::Task(task) | Place::TaskConversation(task) => {
      here.and_then(|shown| task_key(shown, *task)).map(|key| key.to_string())
    }
    place => Some(ctx.t(screen_of(*place).title_key()).to_string()),
  };
  Some(match place {
    Some(place) => ctx
      .t_args("shell.operations.where", [("project", name), ("place", place.as_str())])
      .to_string(),
    None => name.to_owned(),
  })
}

const fn screen_of(place: Place) -> Screen {
  match place {
    Place::Overview => Screen::Overview,
    Place::Tasks | Place::Task(_) | Place::TaskConversation(_) => Screen::Tasks,
    Place::PullRequests => Screen::PullRequests,
    Place::Pipeline(_) => Screen::Pipelines,
    Place::Instruction(_) => Screen::Instructions,
    Place::Skill(_) => Screen::Skills,
    Place::AgentProfile(_) => Screen::Agents,
    Place::Tool(_) => Screen::Tools,
    Place::Secrets => Screen::Secrets,
    Place::Connections => Screen::Connections,
    Place::Settings => Screen::ProjectSettings,
  }
}
