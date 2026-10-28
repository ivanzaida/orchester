//! A linked project's Sharing (`04b Sharing · shared` `TnUGj`, *Removed from the project* in `N9NFio`): *Team server*
//! with its address and the person's role ([`rows`]) and its members read-only ([`super::members`]), *What a shared
//! project can't do yet*, its discovery file ([`discovery`]), and *Leave*. *Make local* asks in place
//! ([`super::make_local`]); *Leave* asks in place and keeps a local copy of the tasks and this computer's connection to
//! the server; it is done on the server, so it is unavailable, with why, while this computer is not signed in there or
//! it can't be reached. Once the person is no longer a member, the page says so instead of offering *Leave*, and *Make
//! local* stays.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_api::MemberRole;
use orchester_workspace::sharing::{
  Connection, ProjectAddress, ServerEntry,
  project::{MemberChange, ProjectServer},
};

mod discovery;
pub(super) mod joined;
mod removed;
mod rows;

use super::{Facts, make_local, members};
use crate::{
  project::ShownProject,
  screens::{
    project_settings::kit::{Group, SettingRow, question, value},
    settings::orchester::{PersonOnly, Said, sharing_said},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    unavailable::reason_line,
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub facts: Facts,
  pub revision: Signal<u64>,
  pub made_local: Signal<make_local::Made>,
  /// Opens with *Make local*'s question.
  pub ask_make_local: bool,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.facts == other.facts
  }
}

impl DevtoolsInspectable for Props {}

pub(super) struct Linked {
  asking: Signal<bool>,
  /// *Make local*'s question is open.
  making_local: Signal<bool>,
  leave: FutureAction<(), (), Said>,
  /// Puts away what the join left to say.
  dismiss: FutureAction<(), (), Said>,
  /// Leaving is a person's (review 1, M1).
  person: PersonOnly,
}

/// *Leave*, which only a person presses.
const LEAVE: &str = "sharing-leave-confirm";

/// What a shared project's server refuses rather than loses (`check_merging` in `orchester_workspace::tasks::writer`).
const REFUSED: usize = 2;

impl Component for Linked {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let (servers, root, revision) = (
      Services::of(ctx).servers,
      props.project.open.repo_root.clone(),
      props.revision.clone(),
    );
    let (workspace, project, again) = (
      Services::of(ctx).workspace,
      props.project.open.project.id(),
      props.revision.clone(),
    );
    let dismiss = ctx.future_action(move |(): ()| {
      let (workspace, again) = (workspace.clone(), again.clone());
      async move {
        orchester_workspace::background::run(move || joined::dismiss(&workspace, project))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?;
        again.update(|count| *count += 1);
        Ok(())
      }
    });
    Self {
      dismiss,
      asking: ctx.signal(false),
      making_local: ctx.signal(props.ask_make_local),
      leave: ctx.future_action(move |(): ()| {
        let (servers, root, revision) = (servers.clone(), root.clone(), revision.clone());
        async move {
          let server = ProjectServer::open(&servers, &root)
            .map_err(|error| sharing_said(&error))?
            .ok_or_else(|| Said::new("sharing.linked.notLinked"))?;
          let me = server.me().ok_or_else(|| Said::new("cloud.errors.serverSignedOut"))?;
          server
            .change_member(MemberChange::Remove(me))
            .await
            .map_err(|error| sharing_said(&error))?;
          revision.update(|count| *count += 1);
          Ok(())
        }
      }),
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let Some(link) = props.facts.link.as_ref() else {
      return Column::new();
    };
    let server = props
      .facts
      .linked_server()
      .map_or_else(|| missing_server(ctx, link), |entry| entry.stored.name.clone());
    let name = props.project.open.project.name().as_str().to_owned();
    let entry = props.facts.linked_server();
    // The server project's name as the server answered; this project's own until it has (`Kontur` in both, in the
    // design).
    let shared = rows::shared_name(ctx, link, entry).unwrap_or_else(|| name.clone());
    let role = match link.removed_at {
      Some(_) => removed::role_row(ctx, link, &shared),
      None => {
        let words = role_words(ctx, link.role);
        rows::role_row(ctx, link, &words)
      }
    };
    let team = Group::new("project-settings-sharing-team", ctx.t("sharing.team.title"))
      .row(rows::address_row(ctx, link, (entry, &server)))
      .row(role)
      .row(members_rows(ctx, &props, &shared, &server));
    let discovery = ctx.mount_keyed::<discovery::DiscoveryRow>(
      "sharing-discovery-row",
      discovery::Props {
        project: props.project.clone(),
        address: link.address.clone(),
        server: entry.map(|entry| (entry.stored.name.clone(), entry.address.clone())),
        discovery: props.facts.linked_discovery.clone(),
        revision: props.revision.clone(),
      },
    );
    let servers = Services::of(ctx).servers;
    let notice = link.removed_at.map(|_| {
      let username = props
        .facts
        .linked_server()
        .and_then(|entry| entry.stored.account.as_ref())
        .map(|account| account.username.clone());
      let names = removed::Names {
        local: &name,
        shared: &shared,
        server: &server,
        tasks: props.facts.tasks,
        username: username.as_deref(),
      };
      removed::notice(ctx, (link, &names), (&servers, &self.making_local))
    });
    let (joined_note, keys) = self.joined(ctx, &props, (link.key.as_str(), &server));
    Column::new()
      .id("project-settings-sharing")
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .with_children(joined_note)
      .with_children(notice)
      .child(team)
      .child(refused(ctx))
      .child(discovery)
      .with_children(keys)
      .child(self.leave_group(ctx, &props, (&shared, &server), link.key.as_str()))
  }
}

impl Linked {
  /// What the join did, until dismissed, and *Keys before joining*, for a project that joined.
  fn joined(&self, ctx: &mut Ctx, props: &Props, (key, server): (&str, &str)) -> (Option<Element>, Option<Element>) {
    let note = props.facts.joined_note.as_ref().map(|note| {
      let dismiss = self.dismiss.clone();
      joined::callout(
        ctx,
        (props.project.open.project.id(), key, server),
        (note, &props.facts.joined),
        move || dismiss.run(()),
      )
    });
    let keys = (!props.facts.joined.is_empty()).then(|| joined::keys(ctx, &props.facts.joined, key));
    (note, keys)
  }

  /// *Leave* (`Leave` `JTiac`): *Leave the project* and *Make this project local*, each asked in place. `shared` is
  /// the server project's name.
  fn leave_group(&self, ctx: &mut Ctx, props: &Props, (shared, server): (&str, &str), key: &str) -> Element {
    let make_local = ctx.mount_keyed::<make_local::MakeLocal>(
      "sharing-make-local-component",
      make_local::Props {
        project: props.project.clone(),
        server: server.to_owned(),
        key: key.to_owned(),
        made: props.made_local.clone(),
        revision: props.revision.clone(),
        asking: self.making_local.clone(),
      },
    );
    let removed = props.facts.link.as_ref().is_some_and(|link| link.removed_at.is_some());
    if removed {
      return Group::new(
        "project-settings-sharing-leave",
        ctx.t("sharing.linked.makeLocalSection"),
      )
      .row(make_local)
      .into();
    }
    let leave = self.leave_row(ctx, props, (shared, server));
    Group::new("project-settings-sharing-leave", ctx.t("sharing.linked.leaveSection"))
      .row(leave)
      .row(make_local)
      .into()
  }

  /// *Leave the project* with *Leave*, unavailable with why while the server can't do it, and its question in place.
  fn leave_row(&self, ctx: &mut Ctx, props: &Props, (shared, server): (&str, &str)) -> Element {
    let state = self.leave.state().get();
    let entry = props.facts.linked_server();
    let unavailable = leave_unavailable(ctx, entry, server);
    let button = ActionButton::new(
      "sharing-leave",
      ButtonKind::DangerGhost,
      ctx.t("sharing.linked.leaveButton"),
    )
    .icon(Icon::LogOut)
    .in_form();
    let row = SettingRow::new("sharing-leave-row", ctx.t("sharing.team.leave"))
      .hint(ctx.t("sharing.linked.leaveHint"))
      .info(ctx.t_args(
        // Without the server in the list, this computer is not connected to it for anything else either.
        if entry.is_some() {
          "sharing.linked.leaveDescription"
        } else {
          "sharing.linked.leaveDescriptionNoServer"
        },
        [("server", server)],
      ));
    let row = match &unavailable {
      Some(reason) => row
        .control(button.unavailable(reason.clone()))
        .under(reason_line("sharing-leave-unavailable", reason)),
      None if self.asking.get() => row.under(self.question(ctx, (shared, server), state.is_pending())),
      None => {
        let asking = self.asking.clone();
        row.control(button.on_click(move || asking.set(true)))
      }
    };
    let error = state
      .error
      .clone()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("sharing-leave-error", &reason.text(ctx)).into());
    row
      .under_all(self.person.reason(ctx, LEAVE))
      .under_all(error)
      .render(ctx)
  }

  /// *Leave Kontur?* in place.
  fn question(&self, ctx: &Ctx, (name, server): (&str, &str), pending: bool) -> Element {
    let (leave, asking, person) = (self.leave.clone(), self.asking.clone(), self.person.clone());
    let confirm = ActionButton::new(LEAVE, ButtonKind::Danger, ctx.t("sharing.linked.leaveConfirm")).icon(Icon::LogOut);
    let confirm = if pending {
      confirm.loading()
    } else {
      confirm.on_click(move || {
        if person.allows(LEAVE) {
          asking.set(false);
          leave.run(());
        }
      })
    };
    let keep = self.asking.clone();
    let keep = ActionButton::new(
      "sharing-leave-keep",
      ButtonKind::Ghost,
      ctx.t("sharing.linked.leaveKeep"),
    )
    .on_click(move || keep.set(false));
    question(
      "sharing-leave-question",
      &ctx.t_args("sharing.linked.leaveQuestion", [("project", name)]),
      &[ctx.t_args("sharing.linked.leaveBody", [("project", name), ("server", server)])],
      vec![confirm.into(), keep.into()],
    )
    .into()
  }
}

/// *What a shared project can't do yet* (`CkP8s`): the task fields its server refuses rather than loses, *Delivers* and
/// *Don't merge automatically*.
fn refused(ctx: &mut Ctx) -> Element {
  let row = SettingRow::new("sharing-refused", ctx.t("sharing.team.refusedFields"))
    .hint(ctx.t("sharing.team.refusedFieldsHint"))
    .info(ctx.t("sharing.team.refusedFieldsInfo"))
    .control(value("sharing-refused-count", &REFUSED.to_string()))
    .render(ctx);
  Group::new("project-settings-sharing-refused", ctx.t("sharing.team.refused"))
    .row(row)
    .into()
}

/// The person's role there, as the page says it.
fn role_words(ctx: &Ctx, role: Option<MemberRole>) -> String {
  match role {
    Some(MemberRole::Owner) => ctx.t("sharing.linked.owner"),
    Some(MemberRole::Member) => ctx.t("sharing.linked.member"),
    None => ctx.t("sharing.linked.noRole"),
  }
  .to_string()
}

/// *Members*, read-only, with who the person is there: rows of *Team server*.
fn members_rows(ctx: &mut Ctx, props: &Props, shared: &str, server: &str) -> Element {
  let link = props.facts.link.as_ref();
  let entry = props.facts.linked_server();
  let account = entry.and_then(|entry| entry.stored.account.as_ref());
  ctx.mount_keyed::<members::Members>(
    "sharing-members",
    members::Props {
      project: props.project.clone(),
      server: server.to_owned(),
      shared: shared.to_owned(),
      address: link.and_then(|link| link.address.clone()),
      entry: entry.cloned(),
      owner: link.is_some_and(|link| link.role == Some(MemberRole::Owner)),
      admin: account.is_some_and(|account| account.admin),
      me: account.map(|account| account.user),
      connected: entry.is_some_and(|entry| entry.connection == Some(Connection::Connected)),
      removed: link.and_then(|link| link.removed_at),
      username: account.map(|account| account.username.clone()),
    },
  )
}

/// What the page calls the project's server once it left the list: the host of the project's address, which the
/// person can connect to again.
fn missing_server(ctx: &Ctx, link: &orchester_store::SharingLink) -> String {
  link
    .address
    .as_deref()
    .and_then(|address| ProjectAddress::parse(address).ok())
    .map_or_else(
      || ctx.t("sharing.linked.unknownServer").to_string(),
      |address| address.server().authority(),
    )
}

/// Why *Leave* cannot be pressed now: it is done on the server, by the person signed in there, so it needs the server
/// in the list, a sign-in and an answer (as *Manage* in `q8DIyg`). *Make local* works without them.
fn leave_unavailable(ctx: &Ctx, entry: Option<&ServerEntry>, server: &str) -> Option<Arc<str>> {
  let server = [("server", server)];
  match entry.filter(|entry| entry.in_use()).map(|entry| entry.connection) {
    None | Some(Some(Connection::SignedOut)) => Some(ctx.t_args("sharing.linked.leaveNoServer", server)),
    Some(Some(Connection::Connected)) => None,
    Some(_) => Some(ctx.t_args("sharing.linked.leaveOffline", server)),
  }
}
