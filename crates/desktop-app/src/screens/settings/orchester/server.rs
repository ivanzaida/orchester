//! A team server's entry (`Server details` `v9NL4N` in `SFaAp`; its states in `Aowlk`): the header with *Sign out…*,
//! which asks in place and names the projects it affects; a changed certificate to compare (person only) or an ended
//! sign-in to join again; the server's facts; the person's projects there with their checkouts here; *Add a computer*;
//! and the server admin, *Open server admin* (a link) and *Sign in to the admin* (a pairing code). The pairing code
//! shows once.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_api::certificate::CertificatePin;
use orchester_workspace::sharing::{
  Connection, LinkedProject, PresentedCertificate, ServerEntry, ServerProject, cloud::ServerJoin,
};

use super::{
  Said,
  code::{Purpose, Secret, Shown, shown_code},
  parts::{Fact, body, frame, header},
  server_parts, server_projects,
  words::{cloud_error, server_look, sharing_error},
};
use crate::{
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub entry: ServerEntry,
  pub linked: Vec<LinkedProject>,
  /// Changes whenever the servers or the cloud account changed, so the projects are read again.
  pub revision: u64,
}

impl DevtoolsInspectable for Props {}

/// The person's projects on the server, as it answered.
#[derive(Clone, Debug)]
pub(super) struct Projects(pub Vec<ServerProject>);

impl PartialEq for Projects {
  fn eq(&self, other: &Self) -> bool {
    self.0.len() == other.0.len()
      && self
        .0
        .iter()
        .zip(&other.0)
        .all(|(one, two)| one.project.id == two.project.id && one.checkout == two.checkout)
  }
}

impl DevtoolsInspectable for Projects {}

/// The certificate the server presents now, read for the person to compare after it changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Presented(pub PresentedCertificate);

impl DevtoolsInspectable for Presented {}

pub(super) struct ServerPanel {
  pub(super) confirming: Signal<bool>,
  /// Whether the pairing code the last request made shows; *Done* hides it.
  pub(super) showing: Signal<bool>,
  pub(super) sign_out: FutureAction<(), (), Said>,
  pub(super) pairing: FutureAction<Purpose, Shown, Said>,
  pub(super) trust: FutureAction<Presented, (), Said>,
  pub(super) join_again: FutureAction<(), JoinedAgain, Said>,
  pub(super) opened: Signal<Option<Said>>,
}

/// How joining again with the cloud account ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum JoinedAgain {
  Joined,
  /// The server waits for another computer of the person's, or an admin, to approve this one.
  Waiting,
  /// The server has no place for the account.
  NotInvited,
}

impl DevtoolsInspectable for JoinedAgain {}

impl Component for ServerPanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let props = ctx.props::<Props>().clone();
    let (servers, entry) = (services.servers.clone(), props.entry.clone());
    let sign_out = ctx.future_action(move |(): ()| {
      let (servers, entry) = (servers.clone(), entry.clone());
      async move { servers.sign_out(&entry).await.map_err(|error| sharing_error(&error)) }
    });
    let (servers, entry) = (services.servers.clone(), props.entry.clone());
    let pairing = ctx.future_action(move |purpose: Purpose| {
      let (servers, entry) = (servers.clone(), entry.clone());
      async move {
        let code = servers
          .pairing_code(&entry)
          .await
          .map_err(|error| sharing_error(&error))?;
        Ok(Shown {
          purpose,
          server: entry.stored.name.clone(),
          host: entry.address.authority(),
          username: entry
            .stored
            .account
            .as_ref()
            .map(|account| account.username.clone())
            .unwrap_or_default(),
          code: Secret::new(&code.code),
        })
      }
    });
    let (servers, entry) = (services.servers.clone(), props.entry.clone());
    let trust = ctx.future_action(move |presented: Presented| {
      let (servers, entry) = (servers.clone(), entry.clone());
      async move {
        servers
          .trust_again(&entry, presented.0.confirm())
          .await
          .map_err(|error| sharing_error(&error))
      }
    });
    let (cloud, address) = (services.cloud, props.entry.address);
    let join_again = ctx.future_action(move |(): ()| {
      let (cloud, address) = (cloud.clone(), address.clone());
      async move {
        match cloud.join_server(&address).await.map_err(|error| cloud_error(&error))? {
          ServerJoin::Joined(orchester_workspace::sharing::CloudJoined::Joined(_)) => Ok(JoinedAgain::Joined),
          ServerJoin::Joined(orchester_workspace::sharing::CloudJoined::Waiting(_)) => Ok(JoinedAgain::Waiting),
          ServerJoin::NotInvited => Ok(JoinedAgain::NotInvited),
        }
      }
    });
    Self {
      confirming: ctx.signal(false),
      showing: ctx.signal(false),
      sign_out,
      pairing,
      trust,
      join_again,
      opened: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let pairing = self.pairing.state().get();
    let shown = (pairing.is_fulfilled() && self.showing.get())
      .then_some(pairing.data)
      .flatten();
    let projects = projects(ctx, &props);
    let presented = presented(ctx, &props.entry);
    let entry = &props.entry;
    let look = server_look(entry);
    let facts = [Fact::Plain(entry.address.authority())];
    let mut head = header(
      ctx,
      "orchester-server-header",
      (Icon::Server, &entry.stored.name, &look),
      &facts,
    );
    if entry.in_use() {
      let confirming = self.confirming.clone();
      head = head.child(
        ActionButton::new(
          "orchester-server-sign-out",
          ButtonKind::Secondary,
          ctx.t("cloud.server.signOut"),
        )
        .icon(Icon::LogOut)
        .on_click(move || confirming.set(true)),
      );
    }
    let mut parts = Vec::new();
    parts.extend(server_parts::problems(ctx, self, (&props, presented.as_ref())));
    if self.confirming.get() {
      parts.push(server_parts::sign_out_question(ctx, self, &props));
    }
    if let Some(shown) = shown {
      let showing = self.showing.clone();
      parts.push(shown_code(ctx, shown, move || showing.set(false)));
    }
    parts.push(server_parts::facts(ctx, entry));
    parts.push(server_projects::projects(ctx, &props, projects.as_ref()));
    if entry
      .connection
      .is_some_and(|connection| connection != Connection::SignedOut)
    {
      parts.push(server_parts::computers(ctx, self, entry));
      parts.push(server_parts::admin(ctx, self, entry));
    }
    parts.push(server_parts::token_note(ctx, entry));
    frame("orchester-server-details")
      .child(head)
      .child(body("orchester-server-body", parts))
  }
}

/// The person's projects on the server, read while it is connected. An offline server is not asked: its entry changes
/// with every attempt to reach it, and the projects linked here show without it.
fn projects(ctx: &mut Ctx, props: &Props) -> Option<Result<Projects, Said>> {
  let entry = &props.entry;
  // The words say the projects linked here are listed below only when there are some.
  let none_here = props.linked.is_empty();
  if !entry.in_use() || matches!(entry.connection, Some(Connection::SignedOut)) {
    return Some(Err(Said::new(if none_here {
      "cloud.server.projects.signedOutNoneHere"
    } else {
      "cloud.server.projects.signedOut"
    })));
  }
  if entry.certificate_changed {
    return Some(Err(Said::new("cloud.errors.certificateChanged")));
  }
  match entry.connection {
    Some(Connection::Reconnecting { .. } | Connection::Offline { .. }) => {
      let key = if none_here {
        "cloud.server.projects.offlineNoneHere"
      } else {
        "cloud.server.projects.offline"
      };
      return Some(Err(Said::new(key).with("server", entry.stored.name.clone())));
    }
    Some(Connection::Checking) | None => return None,
    Some(Connection::Connected | Connection::SignedOut) => {}
  }
  let servers = Services::of(ctx).servers;
  let state = ctx
    .future(
      (props.revision, entry.stored.id, servers),
      |(_, server, servers)| async move {
        let entries = servers.clone();
        let entry = orchester_workspace::background::run(move || entries.entries())
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| sharing_error(&error))?
          .into_iter()
          .find(|entry| entry.stored.id == server)
          .ok_or_else(|| Said::new("cloud.errors.local"))?;
        servers
          .server_projects(&entry)
          .await
          .map(Projects)
          .map_err(|error| sharing_error(&error))
      },
    )
    .state()
    .get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(projects), _) => Some(Ok(projects)),
    _ => None,
  }
}

/// The certificate the server presents now, read only after it changed.
fn presented(ctx: &mut Ctx, entry: &ServerEntry) -> Option<Result<Presented, Said>> {
  let changed = entry.certificate_changed.then(|| entry.address.clone());
  let state = ctx
    .future(changed, |changed| async move {
      let Some(address) = changed else {
        return Ok(None);
      };
      PresentedCertificate::read(&address)
        .await
        .map(|presented| Some(Presented(presented)))
        .map_err(|error| super::words::server_client(&error))
    })
    .state()
    .get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(Some(presented)), _) => Some(Ok(presented)),
    _ => None,
  }
}

/// The check code of a pin stored in hex, `3F7A-92C1-KD8M-QX4T`.
pub(super) fn check_code(pin: &str) -> Option<String> {
  CertificatePin::parse(pin).map(|pin| pin.check_code())
}
