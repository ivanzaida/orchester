//! An admin's confirmations (10.08 C, D, E): *Reset …'s password?* signs the person's sessions out
//! and shows the new first password once; *Disable …?* signs them out everywhere and releases their
//! claims; the last admin who can sign in cannot be disabled.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::ElementRef,
  node::{Element, dimension::Dimension},
};
use orchester_api::ErrorCode;

use super::{
  FirstPassword, ServerDialog,
  dialogs::Context,
  frame::{Dialog, button, cancel, dialog},
  users::Person,
};
use crate::{
  messages::Message,
  sharing::{ServerEntry, feed::ServerFeed},
  theme::ExtraSpacing,
  ui::{field::wrapping_field_error, icon::Icon, message_text, property::property},
};

/// What the confirmation is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
  Reset,
  /// `last_admin`: the person is the last admin who can sign in.
  Disable {
    last_admin: bool,
  },
}

#[derive(Clone)]
struct ConfirmProps {
  context: Context,
  entry: ServerEntry,
  person: Person,
  purpose: Purpose,
}

impl PartialEq for ConfirmProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry.stored == other.entry.stored && self.person == other.person && self.purpose == other.purpose
  }
}

impl lurq::app::component::DevtoolsInspectable for ConfirmProps {}

pub(super) fn reset(ctx: &mut Ctx, context: &Context, entry: ServerEntry, person: Person) -> Element {
  mount(ctx, context, entry, person, Purpose::Reset)
}

pub(super) fn disable(
  ctx: &mut Ctx,
  context: &Context,
  entry: ServerEntry,
  person: Person,
  last_admin: bool,
) -> Element {
  mount(ctx, context, entry, person, Purpose::Disable { last_admin })
}

fn mount(ctx: &mut Ctx, context: &Context, entry: ServerEntry, person: Person, purpose: Purpose) -> Element {
  let key = match purpose {
    Purpose::Reset => format!("reset-password-{}", person.id),
    Purpose::Disable { .. } => format!("disable-user-{}", person.id),
  };
  ctx.mount_keyed::<ConfirmDialog>(
    &key,
    ConfirmProps {
      context: context.clone(),
      entry,
      person,
      purpose,
    },
  )
}

/// What a confirmation did: a reset's new first password, or nothing more to show.
#[derive(Clone, Debug, PartialEq)]
enum Confirmed {
  Reset(FirstPassword),
  Disabled,
}

impl lurq::app::component::DevtoolsInspectable for Confirmed {}

/// Why it did not happen; `last_admin` when the server refused to disable the last admin.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Refused {
  message: Message,
  last_admin: bool,
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

type ConfirmAction = FutureAction<(ServerEntry, Person, Purpose), Confirmed, Refused>;

struct ConfirmDialog {
  frame: ElementRef,
}

impl Component for ConfirmDialog {
  type Props = ConfirmProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = confirm_action(ctx);
    let props = ctx.props::<ConfirmProps>().clone();
    ctx.watch(&action.state(), move |state| match (state.is_pending(), &state.data) {
      (false, Some(Confirmed::Reset(shown))) => {
        props.context.changed();
        props
          .context
          .dialog
          .set(ServerDialog::Shown(props.entry.stored.id, shown.clone()));
      }
      (false, Some(Confirmed::Disabled)) => {
        props.context.changed();
        props.context.dialog.set(ServerDialog::Closed);
      }
      _ => {}
    });
    let frame = ElementRef::new();
    ctx.focus(&frame);
    Self { frame }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ConfirmProps>().clone();
    let action = confirm_action(ctx);
    let state = action.state().get();
    let refused = state.error.clone().filter(|_| !state.is_pending());
    let last_admin = matches!(props.purpose, Purpose::Disable { last_admin: true })
      || refused.as_ref().is_some_and(|refused| refused.last_admin);
    let submit = (!last_admin && !state.is_pending()).then(|| {
      let (action, args) = (
        action.clone(),
        (props.entry.clone(), props.person.clone(), props.purpose),
      );
      Arc::new(move || action.run(args.clone())) as Arc<dyn Fn() + Send + Sync>
    });
    let name = props.person.display_name.as_str();
    let server = props.entry.stored.name.as_str();
    let (id, title, explanation, confirm) = match props.purpose {
      Purpose::Reset => (
        "reset-password",
        ctx.t_args("servers.users.resetTitle", [("name", name)]),
        Some(ctx.t_args("servers.users.resetExplanation", [("name", name)])),
        button(
          "reset-password-confirm",
          ctx.t("servers.users.reset"),
          Some(Icon::KeyRound),
          submit.clone(),
        ),
      ),
      Purpose::Disable { .. } => (
        "disable-user",
        ctx.t_args("servers.users.disableTitle", [("name", name)]),
        (!last_admin).then(|| ctx.t_args("servers.users.disableExplanation", [("name", name)])),
        button(
          "disable-user-confirm",
          ctx.t("servers.users.disable"),
          Some(Icon::UserX),
          submit.clone(),
        ),
      ),
    };
    let body = match (props.purpose, last_admin) {
      (Purpose::Reset, _) => None,
      (Purpose::Disable { .. }, true) => Some(
        Column::new()
          .id("disable-user-last-admin")
          .width(Dimension::Pct(100.0))
          .child(wrapping_field_error(
            &ctx.t_args("servers.users.lastAdmin", [("name", name), ("server", server)]),
          ))
          .into(),
      ),
      (Purpose::Disable { .. }, false) => Some(disable_consequences(ctx)),
    };
    let problem = refused
      .filter(|refused| !refused.last_admin)
      .map(|refused| message_text(ctx, &refused.message));
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id,
        title,
        explanation,
        body,
        problem,
        leading: None,
        actions: vec![
          cancel(ctx, "confirm-user-cancel", props.context.close()),
          confirm.primary().render(ctx),
        ],
        submit,
      },
    )
  }
}

/// What disabling does (`KM5pi`): the person's sessions end, their claims are released, and they
/// stay a member of their projects. The server does not tell an admin how many sessions or claims a
/// person has, so the rows say what happens to them.
fn disable_consequences(ctx: &Ctx) -> Element {
  Column::new()
    .id("disable-user-consequences")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .child(property(
      &ctx.t("servers.users.sessions"),
      &ctx.t("servers.users.sessionsEnd"),
    ))
    .child(property(
      &ctx.t("servers.users.claims"),
      &ctx.t("servers.users.claimsReleased"),
    ))
    .child(property(
      &ctx.t("servers.users.projects"),
      &ctx.t("servers.users.projectsStay"),
    ))
    .into()
}

fn confirm_action(ctx: &mut Ctx) -> ConfirmAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, person, purpose): (ServerEntry, Person, Purpose)| {
    let servers = servers.clone();
    async move {
      let done = match purpose {
        Purpose::Reset => servers
          .reset_password(&entry, person.id)
          .await
          .map(|reset| Confirmed::Reset(FirstPassword::new(person.clone(), reset.initial_password))),
        Purpose::Disable { .. } => servers
          .set_disabled(&entry, person.id, true)
          .await
          .map(|_| Confirmed::Disabled),
      };
      done.map_err(|error| {
        tracing::error!(%error, server = %entry.stored.id, "the admin's change was not made");
        Refused {
          last_admin: error.code() == Some(ErrorCode::LastAdmin),
          message: error.message(&entry.address),
        }
      })
    }
  })
}
