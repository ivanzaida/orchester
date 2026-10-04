//! Settings › Servers (10.01 `sXIz1`, signed out 10.06 `YMwe5`, users 10.07 `BWEZ9`): the
//! application's settings page, with the project's sidebar. The servers are listed at the left and
//! the selected one's details at the right, with its *Account* and, for admins, *Users*. *Add server*
//! (10.02, 10.03) checks an address before it is kept; signing in (10.04, 10.05) keeps the token in
//! the OS keychain. The status bar's server item opens this page at its server, and its *Sign in*
//! opens the sign-in dialog here.

mod account;
mod add;
mod add_result;
mod admin_dialogs;
mod details;
mod frame;
mod labels;
mod list;
mod password;
mod sessions;
mod sign_in;
mod table;
mod user_dialogs;
mod users;

use std::sync::Arc;

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::{Column, Modal, ModalTarget, Row, Text},
  core::{Sensitive, Signal},
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, dimension::Dimension},
};
use orchester_api::Credential;
use orchester_core::id::ServerId;

use self::{
  details::{DetailsProps, ServerDetails},
  users::Person,
};
use super::{
  ProjectNav,
  page::{self, TOPBAR_BUTTON_HEIGHT},
};
use crate::{
  navigation::route::{Page, Route, ServerTab},
  sharing::{
    ServerEntry,
    feed::{ServerFeed, ServerList, SignInRequest},
  },
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    icon::Icon,
    message_text,
    panel::RecoveryPanel,
    tab::{Tab, tab_row},
  },
};

/// Tab row height (`QNAPD`).
const TAB_ROW_HEIGHT: f32 = 42.0;

/// A first password shown once (10.08 B), after creating a user or resetting their password.
#[derive(Clone)]
pub struct FirstPassword {
  pub person: Person,
  /// Shared by the dialog's text and its Copy. `Sensitive` keeps it out of DevTools' signal values
  /// and history and the props inspector; `Credential` wipes it when its last holder drops it, as
  /// the dialog closes.
  password: Arc<Sensitive<Credential>>,
}

impl FirstPassword {
  pub fn new(person: Person, password: Credential) -> Self {
    Self {
      person,
      password: Arc::new(Sensitive::new(password)),
    }
  }

  pub fn password(&self) -> &Arc<Sensitive<Credential>> {
    &self.password
  }
}

impl PartialEq for FirstPassword {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.password, &other.password)
  }
}

impl std::fmt::Debug for FirstPassword {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("FirstPassword")
      .field("person", &self.person.username)
      .finish_non_exhaustive()
  }
}

/// Which dialog is open.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerDialog {
  Closed,
  Add,
  SignIn(ServerId),
  /// The person's own password; `others` names the sessions it signs out.
  ChangePassword(ServerId, Arc<[String]>),
  CreateUser(ServerId),
  Shown(ServerId, FirstPassword),
  ResetPassword(ServerId, Person),
  /// `last_admin` when the person is the last admin who can sign in (10.08 E).
  Disable(ServerId, Person, bool),
}

impl lurq::app::component::DevtoolsInspectable for ServerDialog {}

pub struct ServersScreen {
  dialog: Signal<ServerDialog>,
  /// Bumped after a change, to read the sessions and users again.
  revision: Signal<u64>,
  nav: ProjectNav,
  list_scroll: ScrollState,
  details_scroll: ScrollState,
}

impl Component for ServersScreen {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    let feed = ServerFeed::of(ctx);
    let dialog = ctx.signal(ServerDialog::Closed);
    let nav = ctx
      .use_context::<ProjectNav>()
      .expect("the project shell provides ProjectNav");
    // The status bar's *Sign in*: this page is open at the server; the dialog opens over it.
    let (request, open, shown) = (feed.sign_in.clone(), dialog.clone(), nav.clone());
    let take = move |requested: SignInRequest| {
      if let SignInRequest(Some(server)) = requested {
        request.set(SignInRequest(None));
        shown.replace(Page::Servers(Some(server), ServerTab::Account));
        open.set(ServerDialog::SignIn(server));
      }
    };
    take(feed.sign_in.get_untracked());
    ctx.watch(&feed.sign_in, move |requested| take(*requested));
    Self {
      dialog,
      revision: ctx.signal(0_u64),
      nav,
      list_scroll: ScrollState::new(),
      details_scroll: ScrollState::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let feed = ServerFeed::of(ctx);
    let list = feed.list.get();
    let (selected, tab) = match Route::current_page(ctx) {
      Some(Page::Servers(selected, tab)) => (selected, tab),
      _ => (None, ServerTab::Account),
    };
    let shown = selected
      .and_then(|server| list.entry(server))
      .or_else(|| list.entries().first())
      .cloned();
    let body = match &list {
      ServerList::Loading => Column::new().flex(1.0).into(),
      ServerList::Failed(error) => centered(
        RecoveryPanel::new(
          Icon::AlertCircle,
          &ctx.t("servers.loadFailed"),
          &message_text(ctx, error),
        )
        .into(),
      ),
      ServerList::Ready(entries) => self.body(ctx, entries, shown.as_ref(), tab),
    };
    let dialog = self.dialog(ctx, &list);
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .child(page::topbar(&ctx.t("servers.breadcrumb"), [self.add_button(ctx)]))
      .child(page::heading(&ctx.t("servers.title"), &ctx.t("servers.description")))
      .child(tabs(ctx, list.entries().len()))
      .child(body)
      .child(dialog)
  }
}

impl ServersScreen {
  fn add_button(&self, ctx: &Ctx) -> Element {
    let dialog = self.dialog.clone();
    ActionButton::new(ctx.t("servers.add.open"))
      .icon(Icon::Plus)
      .id("add-server")
      .height(Dimension::Px(TOPBAR_BUTTON_HEIGHT))
      .on_click(move || dialog.set(ServerDialog::Add))
      .render(ctx)
  }

  fn body(&self, ctx: &mut Ctx, entries: &[ServerEntry], shown: Option<&ServerEntry>, tab: ServerTab) -> Element {
    let list = list::server_list(
      ctx,
      entries,
      shown.map(|entry| entry.stored.id),
      &self.nav,
      &self.list_scroll,
    );
    let details = match shown {
      Some(entry) => ctx.mount_keyed::<ServerDetails>(
        &format!("server-details-{}", entry.stored.id),
        DetailsProps {
          entry: entry.clone(),
          tab,
          dialog: self.dialog.clone(),
          revision: self.revision.get(),
          scroll: self.details_scroll.clone(),
        },
      ),
      None => empty(ctx),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(list)
      .child(Column::new().flex(1.0).height(Dimension::Pct(100.0)).child(details))
      .into()
  }

  /// The open dialog over a backdrop.
  fn dialog(&self, ctx: &mut Ctx, list: &ServerList) -> Element {
    let closed = || Modal::new(Column::new()).open_when(false).into();
    let mode = self.dialog.get();
    let entry = |server: ServerId| list.entry(server).cloned();
    let context = dialogs::Context {
      dialog: self.dialog.clone(),
      revision: self.revision.clone(),
      nav: self.nav.clone(),
    };
    let content = match mode {
      ServerDialog::Closed => return closed(),
      ServerDialog::Add => add::mount(ctx, &context, list),
      ServerDialog::SignIn(server) => match entry(server) {
        Some(entry) => sign_in::mount(ctx, &context, entry),
        // A server just added appears in the list a moment later.
        None => return closed(),
      },
      ServerDialog::ChangePassword(server, others) => match entry(server) {
        Some(entry) => password::mount(ctx, &context, entry, others),
        None => return closed(),
      },
      ServerDialog::CreateUser(server) => match entry(server) {
        Some(entry) => user_dialogs::create(ctx, &context, entry),
        None => return closed(),
      },
      ServerDialog::Shown(server, shown) => match entry(server) {
        Some(entry) => user_dialogs::shown(ctx, &context, &entry, &shown),
        None => return closed(),
      },
      ServerDialog::ResetPassword(server, person) => match entry(server) {
        Some(entry) => admin_dialogs::reset(ctx, &context, entry, person),
        None => return closed(),
      },
      ServerDialog::Disable(server, person, last_admin) => match entry(server) {
        Some(entry) => admin_dialogs::disable(ctx, &context, entry, person, last_admin),
        None => return closed(),
      },
    };
    Modal::new(content)
      .target(ModalTarget::Root)
      .dismiss_on_escape(false)
      .into()
  }
}

/// What every dialog of the page gets: how to close it, to open the next one, and to have the
/// details read again.
pub(super) mod dialogs {
  use std::sync::Arc;

  use lurq::core::Signal;

  use super::ServerDialog;
  use crate::screens::project::ProjectNav;

  #[derive(Clone)]
  pub struct Context {
    pub dialog: Signal<ServerDialog>,
    pub revision: Signal<u64>,
    pub nav: ProjectNav,
  }

  impl Context {
    pub fn close(&self) -> Arc<dyn Fn() + Send + Sync> {
      let dialog = self.dialog.clone();
      Arc::new(move || dialog.set(ServerDialog::Closed))
    }

    /// The change is made: the details read the server again.
    pub fn changed(&self) {
      self.revision.update(|revision| *revision += 1);
    }
  }
}

/// Servers (`QNAPD`) with how many there are.
fn tabs(ctx: &Ctx, count: usize) -> Element {
  tab_row(
    Dimension::Px(TAB_ROW_HEIGHT),
    ExtraSpacing::Step.role(),
    Some(SpacingSize::Section),
    [
      Tab::new("servers-tab-servers", ctx.t("servers.tab"), Dimension::Pct(100.0))
        .selected(true)
        .count(Some(count)),
    ],
  )
}

/// No server yet: what the page is for, beside the empty list.
fn empty(ctx: &Ctx) -> Element {
  Column::new()
    .id("servers-empty")
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Section)
    .child(Text::new(&ctx.t("servers.empty")).variant(ExtraTypography::Summary.role()))
    .into()
}

fn centered(content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .padding(SpacingSize::Section)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
    .into()
}
