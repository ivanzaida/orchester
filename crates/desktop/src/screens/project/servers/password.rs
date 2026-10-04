//! *Change your password* (10.05 E) from Account: the current password and the new one twice; the
//! person's other sessions are signed out when it changes and this computer stays signed in. The new
//! password's fields and their errors (10.05 C, D) are shared with *Choose your password*.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::{Credential, ErrorCode};
use zeroize::Zeroize;

use super::{
  dialogs::Context,
  frame::{Dialog, button, cancel, dialog, error, field, hint, password},
};
use crate::{
  messages::Message,
  sharing::{PASSWORD_CHARS, PasswordProblem, ServerEntry, SharingError, feed::ServerFeed, password_problem},
  theme::ExtraTypography,
  ui::{
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

/// The new password and its repetition, with the rule as a hint and what is wrong under each.
pub(super) fn new_password_fields(
  ctx: &Ctx,
  id: &'static str,
  new: &Signal<String>,
  repeat: &Signal<String>,
) -> Vec<Element> {
  let (typed, repeated) = (new.get(), repeat.get());
  let problem = password_problem(&typed, &repeated);
  let new_note = match problem {
    Some(PasswordProblem::TooShort { chars }) => error(
      format!("{id}-new-error"),
      &ctx.t_args(
        "servers.password.tooShort",
        [
          ("min", PASSWORD_CHARS.start().to_string().as_str()),
          ("count", chars.to_string().as_str()),
        ],
      ),
    ),
    Some(PasswordProblem::TooLong) => error(
      format!("{id}-new-error"),
      &ctx.t_args(
        "servers.password.tooLong",
        [("max", PASSWORD_CHARS.end().to_string().as_str())],
      ),
    ),
    _ => hint(&ctx.t("servers.password.rule")),
  };
  let repeat_note = (problem == Some(PasswordProblem::Different))
    .then(|| error(format!("{id}-repeat-error"), &ctx.t("servers.password.different")));
  let (new_id, repeat_id) = match id {
    "choose" => ("choose-password-new", "choose-password-repeat"),
    _ => ("change-password-new", "change-password-repeat"),
  };
  vec![
    field(&ctx.t("servers.password.new"), password(ctx, new_id, new), [new_note]),
    field(
      &ctx.t("servers.password.repeat"),
      password(ctx, repeat_id, repeat),
      repeat_note,
    ),
  ]
}

/// What to say when a new password was not saved.
pub(super) fn change_message(error: &SharingError, entry: &ServerEntry) -> Message {
  match error.code() {
    Some(ErrorCode::InvalidCredentials) => Message::new("servers.password.wrongCurrent"),
    Some(ErrorCode::Invalid) => Message::new("servers.password.refused"),
    Some(ErrorCode::RateLimited) => Message::new("servers.password.rateLimited"),
    _ => error.message(&entry.address),
  }
}

#[derive(Clone)]
pub(super) struct ChangeProps {
  context: Context,
  entry: ServerEntry,
  others: Arc<[String]>,
}

impl PartialEq for ChangeProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry.stored == other.entry.stored && self.others == other.others
  }
}

impl lurq::app::component::DevtoolsInspectable for ChangeProps {}

pub(super) fn mount(ctx: &mut Ctx, context: &Context, entry: ServerEntry, others: Arc<[String]>) -> Element {
  ctx.mount_keyed::<ChangePasswordDialog>(
    &format!("change-password-{}", entry.stored.id),
    ChangeProps {
      context: context.clone(),
      entry,
      others,
    },
  )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Changed;

impl lurq::app::component::DevtoolsInspectable for Changed {}

type ChangeAction = FutureAction<(ServerEntry, Credential, Credential), Changed, Message>;

struct ChangePasswordDialog {
  /// Made outside `ctx.signal`, so DevTools record no password.
  current: Signal<String>,
  new: Signal<String>,
  repeat: Signal<String>,
  frame: ElementRef,
}

impl Component for ChangePasswordDialog {
  type Props = ChangeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let dialog = Self {
      current: Signal::new(String::new()),
      new: Signal::new(String::new()),
      repeat: Signal::new(String::new()),
      frame: ElementRef::new(),
    };
    let change = change_action(ctx);
    let finish = dialog.finish(&ctx.props::<ChangeProps>().context);
    ctx.watch(&change.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        finish();
      }
    });
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChangeProps>().clone();
    let change = change_action(ctx);
    let state = change.state().get();
    let ready = !self.current.with(String::is_empty) && password_problem(&self.new.get(), &self.repeat.get()).is_none();
    let submit = (ready && !state.is_pending()).then(|| {
      let (action, entry) = (change.clone(), props.entry.clone());
      let (current, new) = (self.current.clone(), self.new.clone());
      Arc::new(move || {
        action.run((
          entry.clone(),
          Credential::new(current.get_untracked()),
          Credential::new(new.get_untracked()),
        ));
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .overflow_visible()
      .child(field(
        &ctx.t("servers.password.current"),
        password(ctx, "change-password-current", &self.current),
        [],
      ))
      .with_children(new_password_fields(ctx, "change", &self.new, &self.repeat))
      .child(others_note(ctx, &props.others));
    let close = self.finish(&props.context);
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id: "change-password",
        title: ctx.t("servers.password.changeTitle"),
        explanation: None,
        body: Some(body.into()),
        problem: state
          .error
          .clone()
          .filter(|_| !state.is_pending())
          .map(|error| message_text(ctx, &error)),
        leading: None,
        actions: vec![
          cancel(ctx, "change-password-cancel", Arc::new(close)),
          button(
            "change-password-save",
            ctx.t("servers.password.change"),
            Some(Icon::KeyRound),
            submit.clone(),
          )
          .primary()
          .render(ctx),
        ],
        submit,
      },
    )
  }
}

impl ChangePasswordDialog {
  fn finish(&self, context: &Context) -> impl Fn() + Send + Sync + 'static {
    let fields = [self.current.clone(), self.new.clone(), self.repeat.clone()];
    let (close, context) = (context.close(), context.clone());
    move || {
      for field in &fields {
        field.update(|text| text.zeroize());
      }
      context.changed();
      close();
    }
  }
}

/// Which other sessions the change signs out (`Iqbfu` in 10.05 E).
fn others_note(ctx: &Ctx, others: &[String]) -> Element {
  let text = match others.len() {
    0 => ctx.t("servers.password.noOthers"),
    1 => ctx.t_args("servers.password.othersOne", [("names", others[0].as_str())]),
    count => ctx.t_args(
      "servers.password.othersMany",
      [
        ("count", count.to_string().as_str()),
        ("names", others.join(&ctx.t("common.listSeparator")).as_str()),
      ],
    ),
  };
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::Monitor, IconSize::Small, PaletteColor::TextSecondary))
    .child(Text::new(&text).variant(ExtraTypography::CellLabel.role()).flex(1.0))
    .into()
}

fn change_action(ctx: &mut Ctx) -> ChangeAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, current, new): (ServerEntry, Credential, Credential)| {
    let servers = servers.clone();
    async move {
      servers
        .change_password(&entry, current, new)
        .await
        .map(|()| Changed)
        .map_err(|error| {
          tracing::info!(%error, server = %entry.stored.id, "the password was not changed");
          change_message(&error, &entry)
        })
    }
  })
}
