//! The shown server's details (`ZU7RB` in 10.01): its name and connection label with *Sign out*, or
//! *Sign in* while nobody is signed in; its address, API and transport in mono; the *Account* and,
//! for an admin, *Users* tabs (`R3v7ow`) over their content.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::{Alignment, layout_kind::ScrollState},
  node::{Element, dimension::Dimension},
};

use super::{
  ServerDialog,
  account::{AccountProps, AccountTab},
  frame::BUTTON_HEIGHT,
  labels::{connection_label, connection_name},
  users::{UsersProps, UsersTab},
};
use crate::{
  messages::Message,
  navigation::route::{Page, ServerTab},
  screens::project::ProjectNav,
  sharing::{Connection, ServerEntry, feed::ServerFeed},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::Icon,
    message_text,
    status_label::status_label,
    tab::{Tab, tab_row},
  },
};

/// Tab row and Sign out heights (`R3v7ow`, `C1x75Y`).
const TAB_ROW_HEIGHT: f32 = 42.0;
const HEADER_BUTTON_HEIGHT: f32 = 32.0;
/// The API version this build speaks; a server speaking another is refused when it is added.
pub(super) const API_VERSION: u8 = 1;

#[derive(Clone)]
pub(super) struct DetailsProps {
  pub entry: ServerEntry,
  pub tab: ServerTab,
  pub dialog: Signal<ServerDialog>,
  /// Changes after a dialog changed something on the server.
  pub revision: u64,
  pub scroll: ScrollState,
}

impl PartialEq for DetailsProps {
  fn eq(&self, other: &Self) -> bool {
    self.entry == other.entry && self.tab == other.tab && self.revision == other.revision
  }
}

impl lurq::app::component::DevtoolsInspectable for DetailsProps {}

/// The server this computer signed out of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SignedOut;

impl lurq::app::component::DevtoolsInspectable for SignedOut {}

type SignOutAction = FutureAction<ServerEntry, SignedOut, Message>;

pub(super) struct ServerDetails {
  nav: ProjectNav,
}

impl Component for ServerDetails {
  type Props = DetailsProps;

  fn create(ctx: &mut Ctx) -> Self {
    sign_out_action(ctx);
    Self {
      nav: ctx
        .use_context::<ProjectNav>()
        .expect("the project shell provides ProjectNav"),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DetailsProps>().clone();
    let sign_out = sign_out_action(ctx);
    let admin = props.entry.stored.account.as_ref().is_some_and(|account| account.admin)
      && props.entry.connection != Some(Connection::SignedOut);
    let tab = if admin { props.tab } else { ServerTab::Account };
    let content = match tab {
      ServerTab::Account => ctx.mount_keyed::<AccountTab>(
        "server-account",
        AccountProps {
          entry: props.entry.clone(),
          dialog: props.dialog.clone(),
          revision: props.revision,
        },
      ),
      ServerTab::Users => ctx.mount_keyed::<UsersTab>(
        "server-users",
        UsersProps {
          entry: props.entry.clone(),
          dialog: props.dialog.clone(),
          revision: props.revision,
        },
      ),
    };
    Column::new()
      .id("server-details")
      .describe("connection", connection_name(props.entry.connection))
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(header(ctx, &props, &sign_out))
      .child(self.tabs(ctx, &props.entry, tab, admin))
      .child(
        ScrollVertical::new(content)
          .with_scroll_state(props.scroll.clone())
          .width(Dimension::Pct(100.0))
          .flex(1.0),
      )
  }
}

impl ServerDetails {
  /// Account and, for an admin, Users (`R3v7ow`); tabs are pointer-only.
  fn tabs(&self, ctx: &Ctx, entry: &ServerEntry, selected: ServerTab, admin: bool) -> Element {
    let height = Dimension::Pct(100.0);
    let server = Some(entry.stored.id);
    let mut tabs = vec![
      Tab::new("server-tab-account", ctx.t("servers.tabs.account"), height)
        .selected(selected == ServerTab::Account)
        .link(self.nav.link(Page::Servers(server, ServerTab::Account))),
    ];
    if admin {
      tabs.push(
        Tab::new("server-tab-users", ctx.t("servers.tabs.users"), height)
          .selected(selected == ServerTab::Users)
          .link(self.nav.link(Page::Servers(server, ServerTab::Users))),
      );
    }
    tab_row(
      Dimension::Px(TAB_ROW_HEIGHT),
      ExtraSpacing::Step.role(),
      Some(SpacingSize::Section),
      tabs,
    )
  }
}

/// Name, label and the account action; the address, API and transport under them (`oxt95`).
fn header(ctx: &Ctx, props: &DetailsProps, sign_out: &SignOutAction) -> Element {
  let entry = &props.entry;
  let (glyph, state) = connection_label(ctx, entry);
  let transport = if entry.address.is_https() {
    ctx.t("servers.details.https")
  } else {
    ctx.t("servers.details.http")
  };
  let api = API_VERSION.to_string();
  let facts = ctx.t_args(
    "servers.details.facts",
    [
      ("address", entry.address.as_str()),
      ("api", api.as_str()),
      ("transport", &*transport),
    ],
  );
  let failure = sign_out.state().get().error;
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding_top(SpacingSize::Lg)
    .padding_horizontal(SpacingSize::Section)
    .padding_bottom(SpacingSize::Md)
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Stack.role())
        .align_items(Alignment::Center)
        .child(Text::new(&entry.stored.name).variant(ExtraTypography::InspectorTitle.role()))
        .child(status_label(glyph, &state))
        .child(Spacer::new().flex(1.0))
        .child(account_action(ctx, props, sign_out)),
    )
    .child(
      Text::new(&facts)
        .id("server-facts")
        .variant(ExtraTypography::PullRequestContext.role()),
    )
    .with_children(failure.map(|error| {
      Column::new()
        .id("server-sign-out-error")
        .child(field_error(&message_text(ctx, &error)))
    }))
    .into()
}

/// *Sign out* while signed in; the primary *Sign in* otherwise (10.04, 10.06).
fn account_action(ctx: &Ctx, props: &DetailsProps, sign_out: &SignOutAction) -> Element {
  let entry = &props.entry;
  let signed_in = entry.in_use() && entry.connection != Some(Connection::SignedOut);
  if signed_in {
    let pending = sign_out.state().get().is_pending();
    let (action, entry) = (sign_out.clone(), entry.clone());
    let button = ActionButton::new(ctx.t("servers.signOut.action"))
      .icon(Icon::LogOut)
      .id("server-sign-out")
      .height(Dimension::Px(HEADER_BUTTON_HEIGHT));
    let button = if pending {
      button.on_click(|| {}).enabled(false)
    } else {
      button.on_click(move || action.run(entry.clone()))
    };
    return button.render(ctx);
  }
  let (dialog, server) = (props.dialog.clone(), entry.stored.id);
  ActionButton::new(ctx.t("servers.signIn.action"))
    .primary()
    .icon(Icon::LogIn)
    .id("server-sign-in")
    .height(Dimension::Px(BUTTON_HEIGHT))
    .on_click(move || dialog.set(ServerDialog::SignIn(server)))
    .render(ctx)
}

/// Signs this computer out of a server at once: no shared project uses it yet, so there is nothing
/// to confirm (10.05 F is for a server a shared project uses).
fn sign_out_action(ctx: &mut Ctx) -> SignOutAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |entry: ServerEntry| {
    let servers = servers.clone();
    async move {
      servers.sign_out(&entry).await.map(|()| SignedOut).map_err(|error| {
        tracing::error!(%error, server = %entry.stored.id, "cannot sign out of the server");
        error.message(&entry.address)
      })
    }
  })
}

/// For the account and users tabs: the entry's admin flag and who it is.
pub(super) fn is_self(entry: &ServerEntry, user: orchester_core::id::UserId) -> bool {
  entry
    .stored
    .account
    .as_ref()
    .is_some_and(|account| account.user == user)
}
