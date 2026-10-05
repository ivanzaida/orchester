//! Sign in (10.04 `X0cARN`, states 10.05 `mcI7H`): username and password; where the password goes
//! and where the token is kept. A wrong username or password reads the same (A); too many attempts
//! wait for the time the server gave (B). A first password from an admin leads to *Choose your
//! password* (C, D), where only *Sign out* and *Save password* work. Typed passwords are masked on
//! screen and in the MCP tree and wiped when the dialog closes.

use std::{
  sync::Arc,
  time::{Duration, SystemTime},
};

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_api::{Credential, ErrorCode};
use zeroize::Zeroize;

use super::{
  dialogs::Context,
  frame::{Dialog, button, cancel, dialog, error, field, hint, input, password},
  password::new_password_fields,
};
use crate::{
  display,
  messages::Message,
  sharing::{
    PendingPasswordChange, ServerEntry, SharingError, SignedIn,
    feed::{ReturnTo, ServerFeed},
    password_problem,
  },
  ui::{icon::Icon, message_text},
};

#[derive(Clone)]
pub(super) struct SignInProps {
  context: Context,
  entry: ServerEntry,
}

impl PartialEq for SignInProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry.stored == other.entry.stored
  }
}

impl lurq::app::component::DevtoolsInspectable for SignInProps {}

pub(super) fn mount(ctx: &mut Ctx, context: &Context, entry: ServerEntry) -> Element {
  ctx.mount_keyed::<SignInDialog>(
    &format!("sign-in-{}", entry.stored.id),
    SignInProps {
      context: context.clone(),
      entry,
    },
  )
}

/// Why signing in did not happen; `until` when the server asks to wait.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Refusal {
  message: Message,
  until: Option<SystemTime>,
}

impl lurq::app::component::DevtoolsInspectable for Refusal {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Done;

impl lurq::app::component::DevtoolsInspectable for Done {}

type SignInAction = FutureAction<(ServerEntry, String, Credential), SignedIn, Refusal>;
type ChooseAction = FutureAction<(ServerEntry, PendingPasswordChange, Credential, Credential), Done, Message>;
type AbandonAction = FutureAction<(ServerEntry, PendingPasswordChange), Done, Message>;

struct SignInDialog {
  username: Signal<String>,
  /// Typed passwords. Made outside `ctx.signal` on purpose: lurq's DevTools record every
  /// component signal with its history, and passwords must not appear there.
  password: Signal<String>,
  new_password: Signal<String>,
  repeat: Signal<String>,
  /// The session signed in with a first password, waiting for the person's own (C).
  pending: Signal<Option<PendingPasswordChange>>,
  frame: ElementRef,
}

impl Component for SignInDialog {
  type Props = SignInProps;

  fn create(ctx: &mut Ctx) -> Self {
    let dialog = Self {
      username: ctx.signal(String::new()),
      password: Signal::new(String::new()),
      new_password: Signal::new(String::new()),
      repeat: Signal::new(String::new()),
      pending: Signal::new(None),
      frame: ElementRef::new(),
    };
    let (sign_in, choose, abandon) = (sign_in_action(ctx), choose_action(ctx), abandon_action(ctx));
    let context = ctx.props::<SignInProps>().context.clone();
    let return_to = ServerFeed::of(ctx).return_to;
    let (finish, pending) = (
      signed_in(dialog.finish(&context), (&return_to, &context)),
      dialog.pending.clone(),
    );
    ctx.watch(&sign_in.state(), move |state| match (state.is_pending(), &state.data) {
      (false, Some(SignedIn::Done)) => finish(),
      (false, Some(SignedIn::PasswordChangeRequired(change))) => pending.set(Some(change.clone())),
      _ => {}
    });
    let finish = signed_in(dialog.finish(&context), (&return_to, &context));
    ctx.watch(&choose.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        finish();
      }
    });
    let finish = dialog.finish(&context);
    ctx.watch(&abandon.state(), move |state| {
      // Signed out on the server or not, the session's token was only in memory and is gone.
      if !state.is_pending() && (state.data.is_some() || state.error.is_some()) {
        finish();
      }
    });
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<SignInProps>().clone();
    let (sign_in, choose, abandon) = (sign_in_action(ctx), choose_action(ctx), abandon_action(ctx));
    match self.pending.get() {
      Some(change) => self.choose(ctx, &props, (&choose, &abandon), change),
      None => self.credentials(ctx, &props, &sign_in),
    }
  }
}

impl SignInDialog {
  /// Wipes every typed password, then closes the dialog and has the page read the server again.
  fn finish(&self, context: &Context) -> impl Fn() + Send + Sync + 'static {
    let fields = [self.password.clone(), self.new_password.clone(), self.repeat.clone()];
    let (pending, close, context) = (self.pending.clone(), context.close(), context.clone());
    move || {
      for field in &fields {
        field.update(|text| text.zeroize());
      }
      pending.set(None);
      context.changed();
      close();
    }
  }

  /// Username and password (10.04, 10.05 A and B).
  fn credentials(&self, ctx: &mut Ctx, props: &SignInProps, sign_in: &SignInAction) -> Element {
    let state = sign_in.state().get();
    let refusal = state.error.clone().filter(|_| !state.is_pending());
    let waiting = refusal
      .as_ref()
      .and_then(|refusal| refusal.until)
      .is_some_and(|until| SystemTime::now() < until);
    let submit = (!state.is_pending()
      && !waiting
      && !self.username.get().trim().is_empty()
      && !self.password.with(String::is_empty))
    .then(|| {
      let (action, entry, username, password) = (
        sign_in.clone(),
        props.entry.clone(),
        self.username.clone(),
        self.password.clone(),
      );
      Arc::new(move || {
        let credential = Credential::new(password.get_untracked());
        action.run((entry.clone(), username.get_untracked().trim().to_owned(), credential));
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let name = props.entry.stored.name.as_str();
    let password_notes = refusal
      .map(|refusal| error("sign-in-error", &message_text(ctx, &refusal.message)))
      .into_iter();
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Md)
      .overflow_visible()
      .child(field(
        &ctx.t("servers.signIn.username"),
        input(ctx, "sign-in-username", &self.username, false),
        [],
      ))
      .child(field(
        &ctx.t("servers.signIn.password"),
        password(ctx, "sign-in-password", &self.password),
        password_notes,
      ))
      .child(hint(&ctx.t_args("servers.signIn.forgot", [("name", name)])));
    let keychain = ctx.t(super::labels::keychain_key());
    let close = self.finish(&props.context);
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id: "sign-in",
        title: ctx.t_args("servers.signIn.title", [("name", name)]),
        explanation: Some(ctx.t_args(
          "servers.signIn.explanation",
          [("address", props.entry.address.as_str()), ("keychain", &*keychain)],
        )),
        body: Some(body.into()),
        problem: None,
        leading: None,
        actions: vec![
          cancel(ctx, "sign-in-cancel", Arc::new(close)),
          button(
            "sign-in-submit",
            ctx.t("servers.signIn.action"),
            Some(Icon::LogIn),
            submit.clone(),
          )
          .primary()
          .render(ctx),
        ],
        submit,
      },
    )
  }

  /// *Choose your password* (10.05 C and D): only Sign out and Save password work.
  fn choose(
    &self,
    ctx: &mut Ctx,
    props: &SignInProps,
    (choose, abandon): (&ChooseAction, &AbandonAction),
    change: PendingPasswordChange,
  ) -> Element {
    let state = choose.state().get();
    let busy = state.is_pending() || abandon.state().get().is_pending();
    let ready = password_problem(&self.new_password.get(), &self.repeat.get()).is_none();
    let submit = (ready && !busy).then(|| {
      let (action, entry, change) = (choose.clone(), props.entry.clone(), change.clone());
      let (current, new) = (self.password.clone(), self.new_password.clone());
      Arc::new(move || {
        let args = (
          entry.clone(),
          change.clone(),
          Credential::new(current.get_untracked()),
          Credential::new(new.get_untracked()),
        );
        action.run(args);
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let sign_out = (!busy).then(|| {
      let (action, entry) = (abandon.clone(), props.entry.clone());
      Arc::new(move || action.run((entry.clone(), change.clone()))) as Arc<dyn Fn() + Send + Sync>
    });
    let server_error = state.error.clone().filter(|_| !state.is_pending());
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Md)
      .overflow_visible()
      .with_children(new_password_fields(ctx, "choose", &self.new_password, &self.repeat));
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id: "choose-password",
        title: ctx.t("servers.password.chooseTitle"),
        explanation: Some(ctx.t_args(
          "servers.password.chooseExplanation",
          [("name", props.entry.stored.name.as_str())],
        )),
        body: Some(body.into()),
        problem: server_error.map(|error| message_text(ctx, &error)),
        leading: Some(
          button(
            "choose-password-sign-out",
            ctx.t("servers.signOut.action"),
            Some(Icon::LogOut),
            sign_out,
          )
          .render(ctx),
        ),
        actions: vec![
          button(
            "choose-password-save",
            ctx.t("servers.password.save"),
            Some(Icon::Check),
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

/// What to say when signing in failed: a wrong username or password the same way for both, and
/// how long to wait after too many attempts.
fn refusal(error: &SharingError, entry: &ServerEntry) -> Refusal {
  match error {
    SharingError::Client(orchester_api::client::ClientError::Api {
      code: ErrorCode::RateLimited,
      retry_after,
      ..
    }) => {
      let wait = Duration::from_secs(retry_after.unwrap_or(60));
      let until = SystemTime::now() + wait;
      let minutes = wait.as_secs().div_ceil(60).to_string();
      Refusal {
        message: Message::new("servers.signIn.rateLimited")
          .with("minutes", minutes)
          .with("time", display::clock_time(until)),
        until: Some(until),
      }
    }
    SharingError::Client(orchester_api::client::ClientError::Api {
      code: ErrorCode::InvalidCredentials,
      ..
    }) => Refusal {
      message: Message::new("servers.signIn.wrong").with("name", entry.stored.name.clone()),
      until: None,
    },
    error => Refusal {
      message: error.message(&entry.address),
      until: None,
    },
  }
}

fn sign_in_action(ctx: &mut Ctx) -> SignInAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, username, password): (ServerEntry, String, Credential)| {
    let servers = servers.clone();
    async move {
      servers.sign_in(&entry, username, password).await.map_err(|error| {
        tracing::info!(%error, server = %entry.stored.id, "signing in did not succeed");
        refusal(&error, &entry)
      })
    }
  })
}

fn choose_action(ctx: &mut Ctx) -> ChooseAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(
    move |(entry, change, current, new): (ServerEntry, PendingPasswordChange, Credential, Credential)| {
      let servers = servers.clone();
      async move {
        servers
          .choose_password(&entry, &change, current, new)
          .await
          .map(|()| Done)
          .map_err(|error| {
            tracing::info!(%error, server = %entry.stored.id, "the new password was not saved");
            super::password::change_message(&error, &entry)
          })
      }
    },
  )
}

fn abandon_action(ctx: &mut Ctx) -> AbandonAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, change): (ServerEntry, PendingPasswordChange)| {
    let servers = servers.clone();
    async move {
      servers.abandon(&entry, &change).await.map(|()| Done).map_err(|error| {
        tracing::warn!(%error, server = %entry.stored.id, "the first-password session was not ended on the server");
        error.message(&entry.address)
      })
    }
  })
}

/// `finish` after a sign-in that went through, returning first to where it was started from (the
/// Sharing tab of a discovery offer), if anywhere.
fn signed_in(
  finish: impl Fn() + Send + Sync + 'static,
  (return_to, context): (&Signal<ReturnTo>, &Context),
) -> impl Fn() + Send + Sync + 'static {
  let (return_to, nav) = (return_to.clone(), context.nav.clone());
  move || {
    if let ReturnTo(Some(page)) = return_to.get_untracked() {
      return_to.set(ReturnTo(None));
      nav.show(page);
    }
    finish();
  }
}
