//! Moving a project between this computer and the account (`LinZt`, `ggjyN`, `xx4ST`; the person's decision,
//! 2026-10-30). While an account is signed in, a row under the pointer offers the move in place of its last two columns
//! (*Move to account*, *Move to this computer*); a press asks in place under the row, as sign-out does (`xQQbr`), and
//! only a person's answer moves it. A shared project does not move: under the pointer its row says so.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_store::RegisteredProject;
use orchester_workspace::{
  background,
  messages::{self, Message},
  project_list::ListedProject,
  sharing::{Place, cloud::SignIn},
  workspace::MoveTo,
};
use parking_lot::Mutex;

use super::Ask;
use crate::{
  screens::settings::orchester::PersonOnly,
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
    pressable::{Focus, pressable},
  },
};

/// The project whose move is asked, and where to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Moving(pub Option<(ProjectId, MoveTo)>);

impl DevtoolsInspectable for Moving {}

/// What a row offers under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Offer {
  Move(MoveTo),
  /// It stays with its server.
  Shared,
  Nothing,
}

/// What `listed`, at `place`, offers: nothing while nobody is signed in or its data is not read.
pub(super) fn offer(signed_in: bool, listed: &ListedProject, place: Option<&Place>) -> Offer {
  if !signed_in || !listed.opens() {
    return Offer::Nothing;
  }
  match place {
    Some(Place::Shared { .. }) => Offer::Shared,
    Some(Place::Local) if listed.project.owner.is_some() => Offer::Move(MoveTo::Computer),
    Some(Place::Local) => Offer::Move(MoveTo::Account),
    None => Offer::Nothing,
  }
}

/// *Move to account* or *Move to this computer*, which the row sets at the end of the room of its last two columns.
pub(super) fn link(ctx: &Ctx, listed: &ListedProject, to: MoveTo, ask: &Ask) -> Element {
  let id = listed.project.id;
  let label = ctx.t(match to {
    MoveTo::Account => "projects.move.toAccount",
    MoveTo::Computer => "projects.move.toComputer",
  });
  let moving = ask.moving.clone();
  Focus::Link
    .apply(pressable(move || moving.set(Moving(Some((id, to))))))
    .id(format!("project-move-{id}"))
    .describe("a11y_name", label.to_string())
    .child(Text::new(&label).variant(ExtraTypography::Prose.role()).nowrap())
    .into()
}

/// *A shared project stays where it is. Make it local first.*, under a shared row (`xx4ST`).
pub(super) fn shared_note(ctx: &Ctx, project: ProjectId) -> Element {
  Row::new()
    .id(format!("project-move-shared-{project}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SpacingSize::Md).bottom(ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Base, PaletteColor::TextMuted))
    .child(Text::new(&ctx.t("projects.move.shared")).variant(TypographyStyle::Description))
    .into()
}

/// The question for `listed`'s move to `to`.
pub(super) fn question(ctx: &mut Ctx, listed: &ListedProject, to: MoveTo, ask: &Ask) -> Element {
  ctx.mount_keyed::<Question>(
    &format!("move-project-{}", listed.project.id),
    QuestionProps {
      project: listed.project.clone(),
      to,
      ask: ask.clone(),
    },
  )
}

#[derive(Clone)]
struct QuestionProps {
  project: RegisteredProject,
  to: MoveTo,
  ask: Ask,
}

impl PartialEq for QuestionProps {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.to == other.to
  }
}

impl DevtoolsInspectable for QuestionProps {}

/// The project to move, and where.
#[derive(Clone, Debug, PartialEq)]
struct Target(RegisteredProject, MoveTo);

impl DevtoolsInspectable for Target {}

/// The project moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Moved;

impl DevtoolsInspectable for Moved {}

const CONFIRM: &str = "move-project-confirm";

struct Question {
  person: PersonOnly,
  /// Whether the move's end is followed already; once per question.
  followed: Mutex<bool>,
}

impl Component for Question {
  type Props = QuestionProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      person: PersonOnly::new(ctx),
      followed: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let action = move_action(ctx);
    self.follow(ctx, &action);
    let props = ctx.props::<QuestionProps>().clone();
    let name = props.project.name.as_str();
    let email = signed_in_email(ctx);
    let (title, body, go, keep, glyph) = match props.to {
      MoveTo::Account => (
        ctx.t_args("projects.move.accountTitle", [("name", name)]),
        ctx.t_args("projects.move.accountBody", [("email", email.as_str())]),
        ctx.t("projects.move.toAccount"),
        ctx.t("projects.move.keepOnComputer"),
        Icon::LogOut,
      ),
      MoveTo::Computer => (
        ctx.t_args("projects.move.computerTitle", [("name", name)]),
        ctx.t("projects.move.computerBody"),
        ctx.t("projects.move.toComputer"),
        ctx.t("projects.move.keepInAccount"),
        Icon::LogIn,
      ),
    };
    let state = action.state().get();
    let failed = state
      .error
      .as_ref()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("move-project-failed", &message_text(ctx, reason)));
    let confirm = ActionButton::new(CONFIRM, ButtonKind::Secondary, go)
      .icon(Icon::ArrowRight)
      .in_form();
    let confirm = if state.is_pending() {
      confirm.loading()
    } else {
      let (action, person, project, to) = (action.clone(), self.person.clone(), props.project.clone(), props.to);
      confirm.on_click(move || {
        if person.allows(CONFIRM) {
          action.run(Target(project.clone(), to));
        }
      })
    };
    let moving = props.ask.moving.clone();
    let keep = ActionButton::new("move-project-keep", ButtonKind::Ghost, keep)
      .in_form()
      .on_click(move || moving.set(Moving(None)));
    let words = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(
        Text::new(&title)
          .id("move-project-title")
          .variant(ExtraTypography::Strong.role()),
      )
      .child(
        Text::new(&body)
          .id("move-project-body")
          .variant(TypographyStyle::Description)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(failed)
      .with_children(self.person.reason(ctx, CONFIRM))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(confirm)
          .child(keep),
      );
    Row::new()
      .id("move-project-question")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Start)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(icon(glyph, IconSize::Lg, PaletteColor::TextSecondary))
      .child(words)
  }
}

impl Question {
  /// Moved: the list is read again, with the project on its new side.
  fn follow(&self, ctx: &mut Ctx, action: &FutureAction<Target, Moved, Message>) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let ask = ctx.props::<QuestionProps>().ask.clone();
    ctx.watch(&action.state(), move |state| {
      if state.is_fulfilled() && !state.is_pending() {
        ask.moving.set(Moving(None));
        ask.revision.update(|revision| *revision += 1);
      }
    });
  }
}

/// The email of the account a project would move into.
fn signed_in_email(ctx: &mut Ctx) -> String {
  match Services::of(ctx).cloud.sign_in_state() {
    SignIn::SignedIn { email, .. } => email,
    SignIn::SignedOut { .. } => String::new(),
  }
}

fn move_action(ctx: &mut Ctx) -> FutureAction<Target, Moved, Message> {
  let servers = Services::of(ctx).servers;
  ctx.future_action(move |Target(project, to): Target| {
    let servers = servers.clone();
    async move {
      background::run(move || servers.move_project(&project, to))
        .await?
        .map(|()| Moved)
        .map_err(|error| messages::move_error(&error))
    }
  })
}
