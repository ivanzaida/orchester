//! Holding new starts from the Tasks list (`docs/design/orchester-workspace.md`, "Pause scheduling"; `Tasks · redesign`
//! `Lw1Gb`): the header's *More actions* (`…`) holds them in the whole project, or releases them, beside Create
//! multiple, which is not built yet and says why; while they are held a warning chip at the end of the views says so
//! with its *Release*. Each reviewed group whose new starts are held is named by its tasks under the views, with its
//! own *Release*. Running runs go on either way (`orchester_workspace::tasks::hold`).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
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
    chip::{BadgeTone, badge},
    field::field_error,
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
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

/// Which part of the holds a mount shows: the header's *More actions* with Create multiple, unavailable for its
/// `reason`; the chip at the end of the views while the whole project's new starts are held; or the held groups.
#[derive(Clone, PartialEq, Eq)]
pub(super) enum QueuePart {
  Menu { reason: Arc<str> },
  Chip,
  Groups,
}

impl DevtoolsInspectable for QueueProps {}

/// A hold to put on new starts, or to release.
#[derive(Clone)]
struct Holding {
  root: PathBuf,
  scope: HoldScope,
  held: bool,
  /// Who pressed: a person, or automation driving the window ([`actor_now`]).
  actor: HoldActor,
}

impl DevtoolsInspectable for Holding {}

/// Whether any of the list's mounts (the menu, the chip, the groups) is changing a hold, so the others wait too; the
/// screen provides it.
#[derive(Clone)]
pub(super) struct Changing(pub Signal<bool>);

pub(super) struct QueueControls {
  hold: FutureAction<Holding, (), Message>,
  changing: Signal<bool>,
  /// The header's menu, while it is open, and its button.
  open: Signal<bool>,
  anchor: ElementRef,
}

impl Component for QueueControls {
  type Props = QueueProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let project = ctx.props::<QueueProps>().project;
    let changing = ctx
      .use_context::<Changing>()
      .expect("the Tasks list provides whether a hold is changing")
      .0;
    let shared = changing.clone();
    let hold = ctx.future_action(
      move |Holding {
              root,
              scope,
              held,
              actor,
            }: Holding| {
        let (changes, changing) = (services.changes.clone(), shared.clone());
        async move {
          changing.set(true);
          let kept: Result<(), Message> = async move {
            let kept = background::run(move || hold_in(&root, (scope, actor), held)).await?;
            // Automation driving the window does not release a person's hold; that says so, as a store error does.
            kept.map_err(HoldRefused::message)?;
            // The list reads the holds again, as after any change to its tasks.
            changes.record(project, Outside::Tasks);
            Ok(())
          }
          .await;
          changing.set(false);
          kept
        }
      },
    );
    Self {
      hold,
      changing,
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<QueueProps>().clone();
    let state = self.hold.state().get();
    let pending = state.is_pending() || self.changing.get();
    let press = |scope: HoldScope, held: bool| {
      let (hold, root) = (self.hold.clone(), props.root.clone());
      move || {
        hold.run(Holding {
          root: root.clone(),
          scope,
          held,
          actor: actor_now(),
        });
      }
    };
    let failed = state.error.as_ref().map(|error| {
      field_error(
        "tasks-queue-error",
        &ctx.t_args("tasks.queue.failed", [("reason", &*message_text(ctx, error))]),
      )
    });
    let shown = match &props.part {
      QueuePart::Groups => return groups(ctx, &props.queue, &press, pending).with_children(failed),
      QueuePart::Menu { reason } => {
        let held = props.queue.project;
        let hold = (!pending).then(|| press(HoldScope::Project, !held));
        self.menu(ctx, (held, hold), reason)
      }
      QueuePart::Chip if props.queue.project => {
        let release = ActionButton::new("tasks-queue-release", ButtonKind::Ghost, ctx.t("tasks.queue.release"))
          .icon(Icon::Play)
          .small();
        Row::new()
          .id("tasks-queue-held")
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(badge(
            "tasks-queue-held-note",
            BadgeTone::Warning,
            &ctx.t("tasks.queue.onHold"),
          ))
          .child(enabled(release, pending, press(HoldScope::Project, false)))
          .into()
      }
      QueuePart::Chip => return Column::new().with_children(failed),
    };
    // Its error shows under it: the header's and the views' rows keep one line until something failed.
    Column::new()
      .id(match props.part {
        QueuePart::Menu { .. } => "tasks-queue",
        _ => "tasks-queue-chip",
      })
      .align_items(Alignment::End)
      .child(shown)
      .with_children(failed)
  }
}

impl QueueControls {
  /// The `…` button and, while it is open, its menu: *Hold new starts*, or *Release new starts* while they are `held`,
  /// which `hold` changes unless a change is under way; and Create multiple, unavailable for `reason`.
  fn menu(
    &self,
    ctx: &mut Ctx,
    (held, hold): (bool, Option<impl Fn() + Send + Sync + 'static>),
    reason: &Arc<str>,
  ) -> Element {
    let open = self.open.clone();
    let button = IconButton::new("tasks-more", Icon::Ellipsis, ctx.t("tasks.more"))
      .element(self.anchor.clone())
      .on_click(move || open.update(|open| *open = !*open));
    let shown = self.open.get();
    let button = if shown { button.active() } else { button }.render(ctx);
    let menu = shown.then(|| {
      let (id, glyph, label) = if held {
        ("tasks-queue-release-all", Icon::Play, ctx.t("tasks.queue.releaseAll"))
      } else {
        ("tasks-queue-hold", Icon::CirclePause, ctx.t("tasks.queue.hold"))
      };
      let item = match hold {
        Some(hold) => MenuItem::new(id, label, hold),
        None => MenuItem::unavailable(id, label, ctx.t("tasks.queue.changing")).without_lock(),
      };
      let create = MenuItem::unavailable("tasks-create-multiple", ctx.t("tasks.createMultiple"), reason.clone())
        .icon(Icon::ListPlus);
      let panel = menu_panel("tasks-more-menu")
        .child(item.icon(glyph).render(ctx, &self.open))
        .child(create.render(ctx, &self.open));
      menu_at_end(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new().child(button).with_children(menu).into()
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

/// Who presses now: lurq's synthetic input (an MCP client driving the window) is automation, held to its rules — it
/// does not release a hold a person set.
pub(crate) fn actor_now() -> HoldActor {
  if is_synthetic_input() {
    HoldActor::Automation
  } else {
    HoldActor::Person
  }
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
