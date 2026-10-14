//! Holding new starts from the Tasks header (`docs/design/orchester-workspace.md`, "Pause scheduling"; no frame draws
//! it, so it keeps to the header's controls): *Hold new starts* holds them in the whole project, and while they are
//! held the header says so with *Release*. Each reviewed group whose new starts are held is named by its tasks, with
//! its own *Release*. Running runs go on either way (`orchester_workspace::tasks::hold`).

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  id::ProjectId,
  task_group::{HoldActor, HoldScope},
};
use orchester_workspace::{
  background,
  messages::Message,
  tasks::{
    hold::{HeldQueue, HoldRefused, hold_in},
    outside::Outside,
  },
};

use crate::{
  services::Services,
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct QueueProps {
  pub(super) project: ProjectId,
  pub(super) root: PathBuf,
  pub(super) queue: HeldQueue,
  pub(super) part: QueuePart,
}

/// Which part of the holds a mount shows: the whole project's, in the header's button row, or the held groups,
/// under the tabs, so the header keeps its layout (`C2E5Po`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum QueuePart {
  Project,
  Groups,
}

impl DevtoolsInspectable for QueueProps {}

/// A hold to put on new starts, or to release.
#[derive(Clone)]
struct Holding {
  root: PathBuf,
  scope: HoldScope,
  held: bool,
}

impl DevtoolsInspectable for Holding {}

pub(super) struct QueueControls {
  hold: FutureAction<Holding, (), Message>,
}

impl Component for QueueControls {
  type Props = QueueProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let project = ctx.props::<QueueProps>().project;
    let hold = ctx.future_action(move |Holding { root, scope, held }: Holding| {
      let changes = services.changes.clone();
      async move {
        let kept = background::run(move || hold_in(&root, (scope, HoldActor::Person), held)).await?;
        // A person's hold is never refused as person-only; a store error says why.
        kept.map_err(|refused| match refused {
          HoldRefused::Store(message) => message,
          HoldRefused::PersonOnly => Message::new("tasks.queue.personHeld"),
        })?;
        // The list reads the holds again, as after any change to its tasks.
        changes.record(project, Outside::Tasks);
        Ok(())
      }
    });
    Self { hold }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<QueueProps>().clone();
    let state = self.hold.state().get();
    let pending = state.is_pending();
    let press = |scope: HoldScope, held: bool| {
      let (hold, root) = (self.hold.clone(), props.root.clone());
      move || {
        hold.run(Holding {
          root: root.clone(),
          scope,
          held,
        });
      }
    };
    let failed = state.error.as_ref().map(|error| {
      field_error(
        "tasks-queue-error",
        &ctx.t_args("tasks.queue.failed", [("reason", &*message_text(ctx, error))]),
      )
    });
    if props.part == QueuePart::Groups {
      return groups(ctx, &props.queue, &press, pending).with_children(failed);
    }
    let project = if props.queue.project {
      let release = ActionButton::new(
        "tasks-queue-release",
        ButtonKind::Secondary,
        ctx.t("tasks.queue.release"),
      )
      .icon(Icon::Play);
      Row::new()
        .id("tasks-queue-held")
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(held_note("tasks-queue-held-note", &ctx.t("tasks.queue.held")))
        .child(enabled(release, pending, press(HoldScope::Project, false)))
    } else {
      let hold =
        ActionButton::new("tasks-queue-hold", ButtonKind::Secondary, ctx.t("tasks.queue.hold")).icon(Icon::CirclePause);
      Row::new().child(enabled(hold, pending, press(HoldScope::Project, true)))
    };
    // Its error shows in place of nothing: the header's row keeps one line.
    match failed {
      Some(failed) => Column::new()
        .id("tasks-queue")
        .align_items(Alignment::End)
        .child(project)
        .child(failed),
      None => Column::new().id("tasks-queue").child(project),
    }
  }
}

/// Each held group, named by its tasks, with its own *Release*.
fn groups<F, P>(ctx: &Ctx, queue: &HeldQueue, press: &P, pending: bool) -> Column
where
  P: Fn(HoldScope, bool) -> F,
  F: Fn() + Send + Sync + 'static,
{
  let rows = queue.groups.iter().enumerate().map(|(index, group)| {
    let named = group_name(ctx, &group.tasks);
    let words = ctx.t_args("tasks.queue.groupHeld", [("tasks", named.as_str())]);
    let release = ActionButton::new(
      format!("tasks-queue-release-group-{index}"),
      ButtonKind::Ghost,
      ctx.t("tasks.queue.release"),
    )
    .small();
    Row::new()
      .id(format!("tasks-queue-group-{index}"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(held_note(&format!("tasks-queue-group-{index}-note"), &words))
      .child(enabled(release, pending, press(HoldScope::Group(group.key), false)))
  });
  Column::new()
    .id("tasks-queue-groups")
    .spacing(ExtraSpacing::Snug.role())
    .with_children(rows)
}

/// `button`, taking presses unless a hold is being kept.
fn enabled(button: ActionButton, pending: bool, press: impl Fn() + Send + Sync + 'static) -> ActionButton {
  if pending {
    button.disabled()
  } else {
    button.on_click(press)
  }
}

/// That new starts are held: a pause mark in `warning` and the words.
fn held_note(id: &str, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CirclePause, IconSize::Sm, PaletteColor::Warning))
    .child(
      Text::new(words)
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
}

/// A group by its tasks: *KON-3, KON-4* or *KON-3, KON-4 and 2 more*.
fn group_name(ctx: &Ctx, tasks: &[String]) -> String {
  const SHOWN: usize = 2;
  let first = tasks.iter().take(SHOWN).cloned().collect::<Vec<_>>().join(", ");
  if tasks.len() <= SHOWN {
    return first;
  }
  let more = (tasks.len() - SHOWN).to_string();
  ctx
    .t_args(
      "tasks.queue.groupMore",
      [("first", first.as_str()), ("more", more.as_str())],
    )
    .to_string()
}
