//! The *Account* tab (`v0qua` in 10.01): who is signed in, their role and since when, *Change
//! password*, *Your sessions* with *Revoke* on every one but this computer's (acting at once), and
//! where the token is kept. Signed out (10.06 `AlrJM`): why, and what comes back after signing in.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::Session;
use orchester_core::id::SessionId;

use super::{
  ServerDialog,
  labels::{day_and_time, keychain_key},
  sessions::{Revoke, sessions_table},
};
use crate::{
  messages::Message,
  sharing::{Connection, ServerEntry, device::Device, feed::ServerFeed},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    message_text,
    notice::Notice,
    property::property,
  },
};

/// Width of the account's properties (`DaByP`) and the Change password height (`gXwzK`).
const PROPERTIES_WIDTH: f32 = 520.0;
const ACTION_HEIGHT: f32 = 32.0;

#[derive(Clone)]
pub(super) struct AccountProps {
  pub entry: ServerEntry,
  pub dialog: Signal<ServerDialog>,
  pub revision: u64,
}

impl PartialEq for AccountProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry == other.entry && self.revision == other.revision
  }
}

impl lurq::app::component::DevtoolsInspectable for AccountProps {}

/// The session that was revoked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Revoked;

impl lurq::app::component::DevtoolsInspectable for Revoked {}

/// The sessions as read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Sessions(pub Vec<SessionRow>);

impl lurq::app::component::DevtoolsInspectable for Sessions {}

/// A session as the table shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SessionRow {
  pub id: SessionId,
  pub device: Device,
  pub created_at: i64,
  pub last_used_at: i64,
  pub current: bool,
}

impl From<Session> for SessionRow {
  fn from(session: Session) -> Self {
    Self {
      id: session.id,
      device: Device::parse(&session.device),
      created_at: session.created_at,
      last_used_at: session.last_used_at,
      current: session.current,
    }
  }
}

pub(super) type RevokeAction = FutureAction<(ServerEntry, SessionId), Revoked, Message>;

pub(super) struct AccountTab {
  /// Bumped after a revoke, to read the sessions again.
  revoked: Signal<u64>,
}

impl Component for AccountTab {
  type Props = AccountProps;

  fn create(ctx: &mut Ctx) -> Self {
    let revoke = revoke_action(ctx);
    let revoked = ctx.signal(0_u64);
    let reload = revoked.clone();
    ctx.watch(&revoke.state(), move |state| {
      if !state.is_pending() && state.data.is_some() {
        reload.update(|count| *count += 1);
      }
    });
    Self { revoked }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AccountProps>().clone();
    let revoke = revoke_action(ctx);
    let column = Column::new()
      .id("server-account")
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Lg)
      .padding_horizontal(SpacingSize::Section)
      .spacing(SpacingSize::Lg);
    let Some(account) = props.entry.stored.account.clone() else {
      return column.child(note(&ctx.t("servers.account.afterSignIn")));
    };
    if props.entry.connection == Some(Connection::SignedOut) {
      return column.child(signed_out_notice(ctx, &props.entry));
    }
    let sessions = self.sessions(ctx, &props);
    let others = match &sessions {
      Some(Ok(Sessions(rows))) => rows
        .iter()
        .filter(|row| !row.current)
        .map(|row| computer(&row.device))
        .collect(),
      _ => Vec::new(),
    };
    let role = if account.admin {
      ctx.t("servers.account.admin")
    } else {
      ctx.t("servers.account.member")
    };
    let signed_in = ctx.t_args(
      "servers.account.signedInAt",
      [("time", &*day_and_time(ctx, account.signed_in_at))],
    );
    let who = ctx.t_args(
      "servers.account.who",
      [
        ("name", account.display_name.as_str()),
        ("username", account.username.as_str()),
      ],
    );
    let properties = Column::new()
      .id("server-account-facts")
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(PROPERTIES_WIDTH))
      .spacing(ExtraSpacing::Stack.role())
      .child(property(&ctx.t("servers.account.signedInAs"), &who))
      .child(property(&ctx.t("servers.account.role"), &role))
      .child(property(&ctx.t("servers.account.since"), &signed_in))
      .child(property(
        &ctx.t("servers.account.sharedProjects"),
        &ctx.t("servers.account.noSharedProjects"),
      ));
    let change = {
      let (dialog, server) = (props.dialog.clone(), props.entry.stored.id);
      let others: std::sync::Arc<[String]> = others.into();
      ActionButton::new(ctx.t("servers.password.change"))
        .icon(Icon::KeyRound)
        .id("server-change-password")
        .height(Dimension::Px(ACTION_HEIGHT))
        .on_click(move || dialog.set(ServerDialog::ChangePassword(server, others.clone())))
        .render(ctx)
    };
    let account_section = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Stack.role())
      .child(Text::new(&ctx.t("servers.account.heading")).variant(ExtraTypography::SectionHeading.role()))
      .child(properties)
      .child(Row::new().padding_top(SpacingSize::Xs).child(change));
    let revoke_press = Revoke {
      action: revoke,
      entry: props.entry.clone(),
    };
    column
      .child(account_section)
      .child(sessions_section(ctx, &account.username, sessions, &revoke_press))
      .child(token_note(ctx, &props.entry))
  }
}

impl AccountTab {
  /// The person's sessions, read again after a revoke or a change elsewhere on the page; `None`
  /// while they are read for the first time.
  fn sessions(&self, ctx: &mut Ctx, props: &AccountProps) -> Option<Result<Sessions, Message>> {
    let servers = ServerFeed::of(ctx).servers;
    let deps = (props.entry.stored.id, props.revision, self.revoked.get());
    let load = ctx.future(deps, move |(server, ..)| {
      let servers = servers.clone();
      async move {
        let entry = current_entry(&servers, server).await?;
        // The page shows the person's admin flag: it follows what the server answers now.
        servers.refresh_account(&entry).await;
        match servers.sessions(&entry).await {
          Ok(sessions) => Ok(Sessions(sessions.into_iter().map(SessionRow::from).collect())),
          Err(error) => Err(error.message(&entry.address)),
        }
      }
    });
    let state = load.state().get();
    match (state.data, state.error) {
      (_, Some(error)) => Some(Err(error)),
      (Some(sessions), None) => Some(Ok(sessions)),
      (None, None) => None,
    }
  }
}

/// `server` as the list has it now. A request's dependencies name the server by its id: its entry
/// changes with every answer (when it was last reached, the account the server reports), which
/// must not read the data again.
pub(super) async fn current_entry(
  servers: &crate::sharing::Servers,
  server: orchester_core::id::ServerId,
) -> Result<ServerEntry, Message> {
  let servers = servers.clone();
  crate::background::run(move || servers.entries())
    .await?
    .map_err(|error| {
      tracing::error!(%error, "cannot read the shared servers");
      Message::new("servers.loadFailed")
    })?
    .into_iter()
    .find(|entry| entry.stored.id == server)
    .ok_or_else(|| Message::new("sharing.errors.serverMissing"))
}

/// The computer a session's device names, for the sessions a password change signs out.
fn computer(device: &Device) -> String {
  match device {
    Device::Orchester { computer, .. } => computer.clone(),
    Device::Other(text) => text.clone(),
  }
}

fn sessions_section(
  ctx: &Ctx,
  username: &str,
  sessions: Option<Result<Sessions, Message>>,
  revoke: &Revoke,
) -> Element {
  let section = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Inline.role())
    .child(Text::new(&ctx.t("servers.sessions.heading")).variant(ExtraTypography::SectionHeading.role()))
    .child(
      Text::new(&ctx.t_args("servers.sessions.description", [("username", username)]))
        .variant(TypographyStyle::Caption)
        .width(Dimension::Pct(100.0)),
    );
  let revoke_error = revoke.action.state().get().error;
  match sessions {
    Some(Ok(Sessions(rows))) => section
      .child(sessions_table(ctx, &rows, revoke))
      .with_children(revoke_error.map(|error| {
        Column::new()
          .id("server-revoke-error")
          .child(field_error(&message_text(ctx, &error)))
      }))
      .into(),
    Some(Err(error)) => section
      .child(
        Column::new()
          .id("server-sessions-error")
          .child(field_error(&message_text(ctx, &error))),
      )
      .into(),
    None => section.into(),
  }
}

/// Where the token is kept, and that Sign out deletes it (`Y5E6K`).
fn token_note(ctx: &Ctx, entry: &ServerEntry) -> Element {
  let keychain = ctx.t(keychain_key());
  Row::new()
    .id("server-token-note")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Lock, IconSize::Small, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args(
        "servers.account.tokenNote",
        [("server", entry.stored.name.as_str()), ("keychain", &*keychain)],
      ))
      .variant(TypographyStyle::Caption)
      .flex(1.0)
      .min_width(Dimension::Px(0.0)),
    )
    .into()
}

/// 10.06: the server no longer accepts this computer's session.
fn signed_out_notice(ctx: &Ctx, entry: &ServerEntry) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(
      Notice::new(
        Icon::LogOut,
        ctx.t_args("servers.signedOut.heading", [("server", entry.stored.name.as_str())]),
        ctx.t("servers.signedOut.detail"),
      )
      .emphasized()
      .id("server-signed-out"),
    )
    .child(note(&ctx.t("servers.account.afterSignIn")))
    .into()
}

fn note(text: &str) -> Element {
  Text::new(text)
    .id("server-account-note")
    .variant(TypographyStyle::Caption)
    .into()
}

fn revoke_action(ctx: &mut Ctx) -> RevokeAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(entry, session): (ServerEntry, SessionId)| {
    let servers = servers.clone();
    async move {
      servers
        .revoke(&entry, session)
        .await
        .map(|()| Revoked)
        .map_err(|error| {
          tracing::error!(%error, server = %entry.stored.id, "cannot revoke a session");
          error.message(&entry.address)
        })
    }
  })
}
