//! The card of a hold the Orchestrator's agent put on new starts (`Action card` `nQIhx`, *New starts held · …*): in
//! the action card's frame, what it held (the whole project, or the group of the task it named), that no waiting
//! task starts until the person resumes, how many tasks wait with their keys, that it was checked as *Pause
//! scheduling* in the queue, and at its start *Resume new starts* (the person's release, as Tasks releases it) and
//! *Open queue* (Tasks).

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::HeldWork,
  task_group::{HoldActor, HoldScope},
  time::Timestamp,
};
use orchester_workspace::{
  background, display,
  messages::Message,
  tasks::hold::{HoldRefused, hold_in},
};

use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route, TaskList},
  services::Services,
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct HeldProps {
  pub(super) id: String,
  pub(super) project: (ProjectId, String),
  pub(super) root: PathBuf,
  /// The task the hold named, whose group it held; `None` for the whole project.
  pub(super) task: Option<String>,
  pub(super) held: HeldWork,
  pub(super) at: Timestamp,
}

impl DevtoolsInspectable for HeldProps {}

/// A release of what the card held.
#[derive(Clone)]
struct Resuming {
  root: PathBuf,
  scope: HoldScope,
  /// Who pressed: a person, or automation driving the window.
  actor: HoldActor,
}

impl DevtoolsInspectable for Resuming {}

pub(super) struct HeldCard {
  resume: FutureAction<Resuming, (), Message>,
}

impl Component for HeldCard {
  type Props = HeldProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let project = ctx.props::<HeldProps>().project.0;
    let resume = ctx.future_action(move |Resuming { root, scope, actor }: Resuming| {
      let changes = services.changes.clone();
      async move {
        let released = background::run(move || hold_in(&root, (scope, actor), false)).await?;
        released.map_err(HoldRefused::message)?;
        changes.record(project, orchester_workspace::tasks::outside::Outside::Tasks);
        Ok(())
      }
    });
    Self { resume }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HeldProps>().clone();
    let id = props.id.as_str();
    let name = props.project.1.as_str();
    let (title, effect) = match &props.task {
      Some(task) => (
        ctx.t_args("orchestrator.held.groupTitle", [("task", task.as_str())]),
        ctx.t("orchestrator.held.groupEffect"),
      ),
      None => (
        ctx.t_args("orchestrator.held.projectTitle", [("project", name)]),
        ctx.t_args("orchestrator.held.projectEffect", [("project", name)]),
      ),
    };
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::CirclePause, IconSize::Md, PaletteColor::TextSecondary))
      .child(
        Text::new(&title)
          .id(format!("{id}-title"))
          .variant(ConversationText::ResultTitle.role())
          .flex_shrink(1.0),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        Row::new()
          .id(format!("{id}-state"))
          .spacing(ExtraSpacing::Pair.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::CircleCheck, IconSize::Sm, PaletteColor::Success))
          .child(
            Text::new(&ctx.t_args(
              "orchestrator.action.confirmed",
              [("time", display::clock(props.at).as_str())],
            ))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
          ),
      );
    let buttons = self.buttons(ctx, &props);
    let failed = self.resume.state().get().error.map(|error| {
      Text::new(&message_text(ctx, &error))
        .id(format!("{id}-error"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Danger)
        .width(Dimension::Pct(100.0))
    });
    Column::new()
      .id(id.to_owned())
      .describe("outcome", "held")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Section.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(head)
      .child(
        Text::new(&effect)
          .id(format!("{id}-effect"))
          .variant(TypographyStyle::Body)
          .width(Dimension::Pct(100.0)),
      )
      .child(waiting(ctx, id, &props.held))
      .child(checked(ctx, name))
      .with_children(failed)
      .child(buttons)
  }
}

impl HeldCard {
  /// *Resume new starts* and *Open queue*, at the card's start.
  fn buttons(&self, ctx: &mut Ctx, props: &HeldProps) -> Row {
    let id = props.id.as_str();
    let resume = ActionButton::new(
      format!("{id}-resume"),
      ButtonKind::Secondary,
      ctx.t("orchestrator.held.resume"),
    )
    .icon(Icon::Play);
    let resume = if self.resume.state().get().is_pending() {
      resume.disabled()
    } else {
      let (action, root) = (self.resume.clone(), props.root.clone());
      let scope = props.held.group.map_or(HoldScope::Project, HoldScope::Group);
      resume.on_click(move || {
        action.run(Resuming {
          root: root.clone(),
          scope,
          actor: crate::screens::tasks::actor_now(),
        });
      })
    };
    let navigation = Navigation::of(ctx);
    let route = Route::Project(props.project.0, ProjectPage::Tasks(TaskList::default()));
    let open = ActionButton::new(
      format!("{id}-open"),
      ButtonKind::Ghost,
      ctx.t("orchestrator.held.openQueue"),
    )
    .icon(Icon::ArrowUpRight)
    .on_click(move || navigation.go(route.clone()));
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(resume)
      .child(open)
  }
}

/// How many tasks wait, and their keys.
fn waiting(ctx: &Ctx, id: &str, held: &HeldWork) -> Column {
  let count = held.waiting.len().to_string();
  let label = if held.waiting.len() == 1 {
    ctx.t("orchestrator.held.waiting.one")
  } else {
    ctx.t_args("orchestrator.held.waiting.other", [("count", count.as_str())])
  };
  let chips = held
    .waiting
    .iter()
    .map(|key| TagChip::new(format!("{id}-task-{key}"), key.as_str()).mono());
  Column::new()
    .id(format!("{id}-waiting"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&label)
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .wrap()
        .with_children(chips),
    )
}

/// That it was checked as *Pause scheduling* in the queue.
fn checked(ctx: &Ctx, project: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::ShieldCheck, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args("orchestrator.held.checked", [("project", project)]))
        .variant(ExtraTypography::Note.role())
        .flex(1.0),
    )
}
