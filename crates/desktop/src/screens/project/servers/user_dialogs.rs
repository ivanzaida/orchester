//! *Create user* (10.08 A, F) and the first password shown once (B): the admin chooses a username,
//! a display name and whether the person is an admin; the server makes the first password, which
//! this dialog shows with *Copy* until *Done*. It is shown nowhere else and never again: handing it
//! to the person is the dialog's purpose (design decision 5).

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  clipboard::copy_to_clipboard,
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::{CreateUser, ErrorCode};

use super::{
  FirstPassword, ServerDialog,
  dialogs::Context,
  frame::{Dialog, button, cancel, dialog, error, field, hint, input},
  users::Person,
};
use crate::{
  messages::Message,
  sharing::{ServerEntry, feed::ServerFeed},
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::{field_error, locked_input, styled_input},
    icon::{Icon, IconSize, icon},
    message_text,
    property::{ValueStyle, styled_property},
    switch::Switch,
  },
};

/// Copy's height in the password box (`t9lYS4`).
const COPY_HEIGHT: f32 = 28.0;
/// Most characters of a username (the server's rule).
const USERNAME_MAX: usize = 64;

#[derive(Clone)]
pub(super) struct CreateProps {
  context: Context,
  entry: ServerEntry,
}

impl PartialEq for CreateProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry.stored == other.entry.stored
  }
}

impl lurq::app::component::DevtoolsInspectable for CreateProps {}

pub(super) fn create(ctx: &mut Ctx, context: &Context, entry: ServerEntry) -> Element {
  ctx.mount_keyed::<CreateUserDialog>(
    &format!("create-user-{}", entry.stored.id),
    CreateProps {
      context: context.clone(),
      entry,
    },
  )
}

/// Why a user was not created; `taken` is the username the server refused as taken.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Refused {
  message: Option<Message>,
  taken: Option<String>,
}

impl lurq::app::component::DevtoolsInspectable for Refused {}

impl lurq::app::component::DevtoolsInspectable for FirstPassword {}

type CreateAction = FutureAction<(ServerEntry, CreateUser), FirstPassword, Refused>;

struct CreateUserDialog {
  username: Signal<String>,
  display_name: Signal<String>,
  admin: Signal<bool>,
  frame: ElementRef,
}

/// Letters, digits, dots, dashes and underscores, as the server accepts; case does not matter.
fn username_valid(username: &str) -> bool {
  !username.is_empty()
    && username.len() <= USERNAME_MAX
    && username
      .bytes()
      .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

impl Component for CreateUserDialog {
  type Props = CreateProps;

  fn create(ctx: &mut Ctx) -> Self {
    let action = create_action(ctx);
    let props = ctx.props::<CreateProps>().clone();
    ctx.watch(&action.state(), move |state| {
      if let (false, Some(shown)) = (state.is_pending(), &state.data) {
        props.context.changed();
        props
          .context
          .dialog
          .set(ServerDialog::Shown(props.entry.stored.id, shown.clone()));
      }
    });
    let dialog = Self {
      username: ctx.signal(String::new()),
      display_name: ctx.signal(String::new()),
      admin: ctx.signal(false),
      frame: ElementRef::new(),
    };
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CreateProps>().clone();
    let action = create_action(ctx);
    let state = action.state().get();
    let username = self.username.get().trim().to_owned();
    let display_name = self.display_name.get().trim().to_owned();
    let refused = state.error.clone().filter(|_| !state.is_pending());
    let taken = refused
      .as_ref()
      .and_then(|refused| refused.taken.as_ref())
      .is_some_and(|taken| taken.eq_ignore_ascii_case(&username));
    let valid = username_valid(&username);
    let submit = (valid && !taken && !display_name.is_empty() && !state.is_pending()).then(|| {
      let (action, entry, admin) = (action.clone(), props.entry.clone(), self.admin.clone());
      let (username, display_name) = (username.clone(), display_name.clone());
      Arc::new(move || {
        action.run((
          entry.clone(),
          CreateUser {
            username: username.clone(),
            display_name: display_name.clone(),
            admin: admin.get_untracked(),
          },
        ));
      }) as Arc<dyn Fn() + Send + Sync>
    });
    let body = self.fields(ctx, &props, &username, (valid, taken));
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id: "create-user",
        title: ctx.t("servers.users.createTitle"),
        explanation: Some(ctx.t("servers.users.createExplanation")),
        body: Some(body),
        problem: refused
          .and_then(|refused| refused.message)
          .map(|message| message_text(ctx, &message)),
        leading: None,
        actions: vec![
          cancel(ctx, "create-user-cancel", props.context.close()),
          button(
            "create-user-submit",
            ctx.t("servers.users.create"),
            Some(Icon::UserPlus),
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

impl CreateUserDialog {
  /// Username with its hint or error, display name with its hint, and the Admin switch.
  fn fields(&self, ctx: &Ctx, props: &CreateProps, username: &str, (valid, taken): (bool, bool)) -> Element {
    let username_note = if taken {
      error(
        "create-user-taken",
        &ctx.t_args(
          "servers.users.taken",
          [("username", username), ("server", props.entry.stored.name.as_str())],
        ),
      )
    } else if !username.is_empty() && !valid {
      error("create-user-invalid", &ctx.t("servers.users.usernameInvalid"))
    } else {
      hint(&ctx.t("servers.users.usernameHint"))
    };
    let admin = self.admin.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .overflow_visible()
      .child(field(
        &ctx.t("servers.users.username"),
        input(ctx, "create-user-username", &self.username, true),
        [username_note],
      ))
      .child(field(
        &ctx.t("servers.users.displayName"),
        input(ctx, "create-user-display-name", &self.display_name, false),
        [hint(&ctx.t("servers.users.displayNameHint"))],
      ))
      .child(
        Switch::new(
          "create-user-admin",
          ctx.t("servers.users.adminOption"),
          self.admin.get(),
        )
        .note(ctx.t("servers.users.adminNote"))
        .render(move || admin.update(|admin| *admin = !*admin)),
      )
      .into()
  }
}

fn create_action(ctx: &mut Ctx) -> CreateAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, user): (ServerEntry, CreateUser)| {
    let servers = servers.clone();
    async move {
      let username = user.username.clone();
      match servers.create_user(&entry, user).await {
        Ok(created) => Ok(FirstPassword {
          person: Person::from(created.user),
          password: Arc::new(created.initial_password),
        }),
        Err(error) if error.code() == Some(ErrorCode::UsernameTaken) => Err(Refused {
          message: None,
          taken: Some(username),
        }),
        Err(error) => {
          tracing::error!(%error, server = %entry.stored.id, "cannot create the user");
          Err(Refused {
            message: Some(match error.code() {
              Some(ErrorCode::Invalid) => Message::new("servers.users.invalid"),
              _ => error.message(&entry.address),
            }),
            taken: None,
          })
        }
      }
    }
  })
}

/// 10.08 B: who can sign in, where, and their first password with Copy, shown only now.
pub(super) fn shown(ctx: &mut Ctx, context: &Context, entry: &ServerEntry, shown: &FirstPassword) -> Element {
  ctx.mount_keyed::<ShownDialog>(
    &format!("first-password-{}", shown.person.id),
    ShownProps {
      context: context.clone(),
      address: entry.address.as_str().into(),
      shown: shown.clone(),
    },
  )
}

#[derive(Clone)]
struct ShownProps {
  context: Context,
  address: Arc<str>,
  shown: FirstPassword,
}

impl PartialEq for ShownProps {
  fn eq(&self, other: &Self) -> bool {
    self.shown == other.shown && self.address == other.address
  }
}

impl lurq::app::component::DevtoolsInspectable for ShownProps {}

struct ShownDialog {
  /// Whether the clipboard took the password, once Copy was pressed. The password itself is not
  /// kept anywhere else.
  copied: Signal<Option<bool>>,
  frame: ElementRef,
}

impl Component for ShownDialog {
  type Props = ShownProps;

  fn create(ctx: &mut Ctx) -> Self {
    let dialog = Self {
      copied: ctx.signal(None),
      frame: ElementRef::new(),
    };
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ShownProps>().clone();
    let person = &props.shown.person;
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .overflow_visible()
      .child(styled_property(
        &ctx.t("servers.users.server"),
        &props.address,
        ValueStyle::Mono,
      ))
      .child(styled_property(
        &ctx.t("servers.users.username"),
        &person.username,
        ValueStyle::Mono,
      ))
      .child(self.password(ctx, &props));
    let done = props.context.close();
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id: "first-password",
        title: ctx.t_args("servers.users.canSignIn", [("name", person.display_name.as_str())]),
        explanation: Some(ctx.t("servers.users.giveExplanation")),
        body: Some(body.into()),
        problem: None,
        leading: None,
        actions: vec![
          button(
            "first-password-done",
            ctx.t("common.done"),
            Some(Icon::Check),
            Some(done.clone()),
          )
          .primary()
          .render(ctx),
        ],
        submit: Some(done),
      },
    )
  }
}

impl ShownDialog {
  /// The first password as a mask the person cannot edit: lurq draws a text either as it is,
  /// which puts it in the MCP and DevTools trees, or masked, which keeps it out of them (rule 11:
  /// no secret in the MCP tree). Copy hands it on. Its signal is made outside `ctx.signal`, so
  /// DevTools record no history of it.
  fn masked(&self, ctx: &Ctx, props: &ShownProps) -> Element {
    let value = Signal::new(props.shown.password.expose().to_owned());
    locked_input(
      styled_input(ctx, value, ExtraTypography::FirstPassword.role())
        .id("first-password-value")
        .single_line()
        .mask()
        .flex(1.0),
      true,
    )
    .into()
  }

  /// The password in mono with Copy (`GMko2`), and that it is shown only now (`vZQ8Z`).
  fn password(&self, ctx: &Ctx, props: &ShownProps) -> Element {
    let outcome = self.copied.get();
    let (copied, password) = (self.copied.clone(), props.shown.password.clone());
    let copy = ActionButton::new(ctx.t(if outcome == Some(true) {
      "common.copied"
    } else {
      "common.copy"
    }))
    .icon(if outcome == Some(true) { Icon::Check } else { Icon::Copy })
    .id("first-password-copy")
    .in_form()
    .height(Dimension::Px(COPY_HEIGHT))
    .on_click(move || {
      let success = copy_to_clipboard(password.expose());
      if !success {
        tracing::warn!("the clipboard did not take the first password");
      }
      copied.set(Some(success));
    })
    .render(ctx);
    let value = Row::new()
      .id("first-password-box")
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .padding_left(ExtraSpacing::Group.role())
      .padding_right(SpacingSize::Sm)
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceBase)
      .border_inside(BorderSize::Sm, ExtraColor::Hairline.role())
      .rounded(RadiusSize::Md)
      .child(self.masked(ctx, props))
      .child(copy);
    let once = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::EyeOff, IconSize::Small, PaletteColor::TextPrimary))
      .child(
        Text::new(&ctx.t("servers.users.shownOnce"))
          .variant(ExtraTypography::Caution.role())
          .flex(1.0),
      );
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .overflow_visible()
      .child(Text::new(&ctx.t("servers.users.firstPassword")).variant(ExtraTypography::FormLabel.role()))
      .child(value)
      .child(once)
      .with_children((outcome == Some(false)).then(|| field_error(&ctx.t("common.copyFailed"))))
      .into()
  }
}
