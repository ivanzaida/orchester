//! Add server (10.02 `e9DDI`, states 10.03 `BWyKt`): the address is checked with `GET /server`
//! before the server is kept. *Continue to sign in* checks an address not checked yet; once the
//! server answered, it keeps the server and opens Sign in. Plain HTTP to another computer needs the
//! opt-in under its warning first; an address already in the list opens that server instead; a
//! redirect, something that is not an Orchester server, a server that cannot be reached or whose
//! certificate is not trusted, a server without an admin and a newer API cannot continue.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, TypographyStyle},
  },
  components::{Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::Element,
};
use orchester_api::client::{PlainHttp, ServerAddress};
use orchester_core::id::ServerId;

use super::{
  ServerDialog,
  add_result::{Answer, Checked, bootstrap_notice, plain_http_warning, result_card, transport_detail},
  details::API_VERSION,
  dialogs::Context,
  frame::{Dialog, button, cancel, dialog, error, field, hint, input},
};
use crate::{
  messages::Message,
  navigation::route::{Page, ServerTab},
  sharing::{ServerEntry, client_message, feed::ServerFeed},
  theme::ExtraSpacing,
  ui::{
    command_row::Copied,
    icon::{Icon, IconSize, icon},
    message_text,
  },
};

#[derive(Clone)]
pub(super) struct AddProps {
  context: Context,
  entries: Arc<[ServerEntry]>,
}

impl PartialEq for AddProps {
  fn eq(&self, other: &Self) -> bool {
    self.entries == other.entries
  }
}

impl lurq::app::component::DevtoolsInspectable for AddProps {}

pub(super) fn mount(ctx: &mut Ctx, context: &Context, list: &crate::sharing::feed::ServerList) -> Element {
  ctx.mount_keyed::<AddServerDialog>(
    "add-server-dialog",
    AddProps {
      context: context.clone(),
      entries: list.entries().into(),
    },
  )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Added(ServerId);

impl lurq::app::component::DevtoolsInspectable for Added {}

type CheckAction = FutureAction<ServerAddress, Checked, Message>;
type AddAction = FutureAction<(ServerAddress, String, PlainHttp), Added, Message>;

/// What the dialog can do with the address as typed.
enum Next {
  /// Nothing to do yet: empty, being checked, or refused.
  Wait,
  Check(ServerAddress),
  Add(ServerAddress, String, PlainHttp),
  Open(ServerId),
}

/// The address a check was started for: its answer counts only while the field still holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Checking(Option<ServerAddress>);

impl lurq::app::component::DevtoolsInspectable for Checking {}

struct AddServerDialog {
  address: Signal<String>,
  opt_in: Signal<bool>,
  checking: Signal<Checking>,
  copied: Signal<Copied>,
  frame: ElementRef,
}

impl Component for AddServerDialog {
  type Props = AddProps;

  fn create(ctx: &mut Ctx) -> Self {
    check_action(ctx);
    let add = add_action(ctx);
    let context = ctx.props::<AddProps>().context.clone();
    ctx.watch(&add.state(), move |state| {
      if let (false, Some(Added(server))) = (state.is_pending(), state.data) {
        context.nav.replace(Page::Servers(Some(server), ServerTab::Account));
        context.dialog.set(ServerDialog::SignIn(server));
      }
    });
    let dialog = Self {
      address: ctx.signal(String::new()),
      opt_in: ctx.signal(false),
      checking: ctx.signal(Checking::default()),
      copied: ctx.signal(Copied::default()),
      frame: ElementRef::new(),
    };
    ctx.focus(&dialog.frame);
    dialog
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<AddProps>().clone();
    let check = check_action(ctx);
    let add = add_action(ctx);
    let typed = self.address.get();
    let parsed = ServerAddress::parse(&typed);
    let known = parsed
      .as_ref()
      .ok()
      .and_then(|address| props.entries.iter().find(|entry| entry.address == *address).cloned());
    // A check of an address the person since changed is left behind: the dialog is idle again.
    let checking = (check.state().get().is_pending() && parsed.as_ref().ok() == self.checking.get().0.as_ref())
      || add.state().get().is_pending();
    let checked = check
      .state()
      .get()
      .data
      .filter(|checked| parsed.as_ref().is_ok_and(|address| *address == checked.address));
    let next = self.next(parsed.as_ref().ok(), known.as_ref(), checked.as_ref(), checking);
    let start = self.start_check(&check);
    let run = run(&next, (&start, &add), &props.context);
    let mut notes = vec![hint(&ctx.t("servers.add.hint"))];
    let mut below = Vec::new();
    match (&known, &checked, &parsed) {
      (Some(known), ..) => notes.push(error(
        "add-server-known",
        &ctx.t_args(
          "servers.add.known",
          [
            ("name", known.stored.name.as_str()),
            ("address", known.address.as_str()),
          ],
        ),
      )),
      (None, _, Err(_)) if !typed.trim().is_empty() => {
        notes.push(error("add-server-invalid", &ctx.t("servers.add.addressInvalid")));
      }
      (None, Some(checked), _) => self.answer(ctx, checked, &mut notes, &mut below),
      _ => {}
    }
    if checking && let Ok(address) = &parsed {
      below.push(checking_line(ctx, address));
    }
    let body = lurq::components::Column::new()
      .width(lurq::node::dimension::Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Md)
      .overflow_visible()
      .child(field(
        &ctx.t("servers.add.address"),
        input(ctx, "add-server-address", &self.address, true),
        notes,
      ))
      .with_children(below);
    let again = parsed
      .as_ref()
      .ok()
      .filter(|_| !checking && known.is_none())
      .map(|address| {
        let (start, address) = (start.clone(), address.clone());
        Arc::new(move || start(address.clone())) as Arc<dyn Fn() + Send + Sync>
      });
    let actions = self.actions(ctx, &props.context, (&next, checked.is_some()), again, run.clone());
    dialog(
      ctx,
      &self.frame,
      Dialog {
        id: "add-server",
        title: ctx.t("servers.add.title"),
        explanation: Some(ctx.t("servers.add.explanation")),
        body: Some(body.into()),
        problem: add.state().get().error.map(|error| message_text(ctx, &error)),
        leading: None,
        actions,
        submit: run,
      },
    )
  }
}

impl AddServerDialog {
  fn next(
    &self,
    address: Option<&ServerAddress>,
    known: Option<&ServerEntry>,
    checked: Option<&Checked>,
    checking: bool,
  ) -> Next {
    if let Some(known) = known {
      return Next::Open(known.stored.id);
    }
    let Some(address) = address.filter(|_| !checking) else {
      return Next::Wait;
    };
    let Some(checked) = checked else {
      return Next::Check(address.clone());
    };
    match &checked.answer {
      Answer::Server {
        name,
        api_version,
        needs_bootstrap: false,
      } if *api_version == API_VERSION => {
        let plain_http = if address.needs_opt_in() {
          if !self.opt_in.get() {
            return Next::Wait;
          }
          PlainHttp::Allowed
        } else {
          PlainHttp::NotAllowed
        };
        Next::Add(address.clone(), name.clone(), plain_http)
      }
      _ => Next::Wait,
    }
  }

  /// The check's answer: the result card and what follows from it, or why it cannot go on.
  fn answer(&self, ctx: &Ctx, checked: &Checked, notes: &mut Vec<Element>, below: &mut Vec<Element>) {
    let address = &checked.address;
    match &checked.answer {
      Answer::Failed(message) => notes.push(error("add-server-error", &message_text(ctx, message))),
      Answer::Server { api_version, .. } if *api_version != API_VERSION => {
        let (theirs, ours) = (api_version.to_string(), API_VERSION.to_string());
        notes.push(error(
          "add-server-newer",
          &ctx.t_args(
            "servers.add.newerApi",
            [("theirs", theirs.as_str()), ("ours", ours.as_str())],
          ),
        ));
      }
      Answer::Server {
        name,
        api_version,
        needs_bootstrap,
      } => {
        let detail = transport_detail(ctx, address, name).filter(|_| !needs_bootstrap);
        below.push(result_card(ctx, name, *api_version, detail));
        if *needs_bootstrap {
          below.push(bootstrap_notice(ctx, name, &self.copied));
        } else if address.needs_opt_in() {
          below.push(plain_http_warning(ctx, address, &self.opt_in));
        }
      }
    }
  }

  /// Cancel; *Check address* before an answer and *Check again* after one; *Continue to sign in*,
  /// available only once the server answered and may be added. An address already listed has
  /// *Open server* instead.
  fn actions(
    &self,
    ctx: &Ctx,
    context: &Context,
    (next, answered): (&Next, bool),
    check: crate::ui::form::Submit,
    run: crate::ui::form::Submit,
  ) -> Vec<Element> {
    let mut actions = vec![cancel(ctx, "add-server-cancel", context.close())];
    if let Next::Open(_) = next {
      actions.push(
        button(
          "add-server-open",
          ctx.t("servers.add.openKnown"),
          Some(Icon::ArrowRight),
          run,
        )
        .render(ctx),
      );
      return actions;
    }
    let (id, label) = if answered {
      ("add-server-check-again", "servers.add.checkAgain")
    } else {
      ("add-server-check", "servers.add.check")
    };
    actions.push(button(id, ctx.t(label), Some(Icon::RefreshCw), check).render(ctx));
    let continue_run = run.filter(|_| matches!(next, Next::Add(..)));
    actions.push(
      button(
        "add-server-continue",
        ctx.t("servers.add.continue"),
        Some(Icon::LogIn),
        continue_run,
      )
      .primary()
      .render(ctx),
    );
    actions
  }

  /// Starts a check of an address, without the opt-in: that is given for the address whose warning
  /// the person read.
  fn start_check(&self, check: &CheckAction) -> Arc<dyn Fn(ServerAddress) + Send + Sync> {
    let (check, opt_in, checking) = (check.clone(), self.opt_in.clone(), self.checking.clone());
    Arc::new(move |address: ServerAddress| {
      opt_in.set(false);
      checking.set(Checking(Some(address.clone())));
      check.run(address);
    })
  }
}

/// What Enter does now: checks an address not checked yet, adds a server that answered, or opens
/// one already listed.
fn run(
  next: &Next,
  (start, add): (&Arc<dyn Fn(ServerAddress) + Send + Sync>, &AddAction),
  context: &Context,
) -> crate::ui::form::Submit {
  match next {
    Next::Wait => None,
    Next::Check(address) => {
      let (start, address) = (start.clone(), address.clone());
      Some(Arc::new(move || start(address.clone())))
    }
    Next::Add(address, name, plain_http) => {
      let (add, args) = (add.clone(), (address.clone(), name.clone(), *plain_http));
      Some(Arc::new(move || add.run(args.clone())))
    }
    Next::Open(server) => {
      let (context, server) = (context.clone(), *server);
      Some(Arc::new(move || {
        context.nav.replace(Page::Servers(Some(server), ServerTab::Account));
        context.dialog.set(ServerDialog::Closed);
      }))
    }
  }
}

/// 10.03 A: the address is being asked what it is.
fn checking_line(ctx: &Ctx, address: &ServerAddress) -> Element {
  Row::new()
    .id("add-server-checking")
    .padding_vertical(lurq::app::theme::SpacingSize::Xs)
    .spacing(ExtraSpacing::Inline.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Loader, IconSize::Dismiss, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args("servers.add.checking", [("host", address.host())])).variant(TypographyStyle::FieldLabel),
    )
    .into()
}

/// Asks the address what it is. Only `GET /server` is sent, which carries nothing of the person,
/// so plain HTTP is allowed for it: the warning and the opt-in come with its answer, before
/// anything else is sent (10.03 B).
fn check_action(ctx: &mut Ctx) -> CheckAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |address: ServerAddress| {
    let servers = servers.clone();
    async move {
      let answer = match servers.check(address.clone(), PlainHttp::Allowed).await {
        Ok(info) => Answer::Server {
          name: info.name,
          api_version: info.api_version,
          needs_bootstrap: info.needs_bootstrap,
        },
        Err(error) => {
          tracing::info!(%error, %address, "the address is not a server this computer can use");
          Answer::Failed(client_message(&error, &address))
        }
      };
      Ok(Checked { address, answer })
    }
  })
}

fn add_action(ctx: &mut Ctx) -> AddAction {
  let servers = ServerFeed::of(ctx).servers;
  ctx.future_action(move |(address, name, plain_http): (ServerAddress, String, PlainHttp)| {
    let servers = servers.clone();
    async move {
      servers
        .add(&address, name, plain_http)
        .await
        .map(Added)
        .map_err(|error| {
          tracing::error!(%error, "cannot add the server");
          error.message(&address)
        })
    }
  })
}
