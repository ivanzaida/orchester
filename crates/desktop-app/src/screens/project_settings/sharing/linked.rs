//! A linked project's Sharing (`L5o9Xy`, *Removed from the project* in `N9NFio`): where its task list lives, its
//! address with the server's state, its repository and discovery file ([`discovery`]), its members read-only
//! ([`super::members`]), and *Leave or make local*. *Make local* asks in place ([`super::make_local`]); *Leave* asks
//! in place and keeps a local copy of the tasks and this computer's connection to the server. Once the person is no
//! longer a member, the page says so instead of offering *Leave* and the members, and *Make local* stays.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_api::MemberRole;
use orchester_workspace::sharing::project::{MemberChange, ProjectServer};

mod discovery;
mod removed;
mod rows;

use super::{Facts, make_local, members};
use crate::{
  project::ShownProject,
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::{PersonOnly, Said, sharing_said},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    form_section::{Label, Place, section, side_row},
    icon::Icon,
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
  /// Leaving is a person's (review 1, M1).
  person: PersonOnly,
}

/// *Leave*, which only a person presses.
const LEAVE: &str = "sharing-leave-confirm";

impl Component for Linked {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let (servers, root, revision) = (
      Services::of(ctx).servers,
      props.project.open.repo_root.clone(),
      props.revision,
    );
    Self {
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
    let server = props.facts.linked_server().map_or_else(
      || ctx.t("sharing.linked.unknownServer").to_string(),
      |entry| entry.stored.name.clone(),
    );
    let name = props.project.open.project.name().as_str().to_owned();
    let role = match link.role {
      Some(MemberRole::Owner) => ctx.t("sharing.linked.owner"),
      Some(MemberRole::Member) => ctx.t("sharing.linked.member"),
      None => ctx.t("sharing.linked.noRole"),
    }
    .to_string();
    let entry = props.facts.linked_server();
    let hint = shown_hint(ctx, (&props, link, entry), &name);
    // The server project's name as the server answered; this project's own until it has (`Kontur` in both, in the
    // design).
    let shared = match &hint {
      Some(Ok(hint)) => hint.name().to_owned(),
      _ => name.clone(),
    };
    let state = match link.removed_at {
      Some(_) => removed::state(ctx, link, &shared),
      None => shared_state(ctx, link, (&server, &role)),
    };
    let task_list = side_row(
      Label::new("sharing-task-list", ctx.t("sharing.link.taskList")).description(ctx.t_args(
        "sharing.linked.taskListDescription",
        [
          ("local", name.as_str()),
          ("project", shared.as_str()),
          ("server", server.as_str()),
        ],
      )),
      state,
      Place::Within,
      true,
    );
    let address_row = rows::address_row(ctx, link, entry);
    let repository_row = rows::repository_row(ctx, &props, hint.as_ref(), (&shared, &server));
    let discovery_row = ctx.mount_keyed::<discovery::DiscoveryRow>(
      "sharing-discovery-row",
      discovery::Props {
        project: props.project.clone(),
        address: link.address.clone(),
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
        .map(|account| account.username.clone())
        .unwrap_or_default();
      let names = removed::Names {
        local: &name,
        shared: &shared,
        server: &server,
        tasks: props.facts.tasks,
        username: &username,
      };
      removed::notice(ctx, (link, &names), (&servers, &self.making_local))
    });
    Column::new()
      .id("project-settings-sharing")
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .with_children(notice)
      .child(section(
        "project-settings-sharing-task-list",
        &ctx.t("sharing.link.taskList"),
        None,
        vec![task_list, address_row, repository_row, discovery_row],
      ))
      .child(members_section(ctx, &props, &shared, &server))
      .child(self.leave_section(ctx, &props, (&shared, &server), link.key.as_str()))
  }
}

impl Linked {
  /// *Leave or make local*: *Make local* and *Leave Kontur*, each asked in place.
  /// `shared` is the server project's name.
  fn leave_section(&self, ctx: &mut Ctx, props: &Props, (shared, server): (&str, &str), key: &str) -> Element {
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
      return section(
        "project-settings-sharing-leave",
        &ctx.t("sharing.linked.makeLocalSection"),
        None,
        vec![make_local],
      )
      .into();
    }
    let state = self.leave.state().get();
    let mut control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm);
    if self.asking.get() {
      control = control.child(self.question(ctx, (shared, server), state.is_pending()));
    } else {
      let asking = self.asking.clone();
      control = control.child(
        Row::new().child(
          ActionButton::new("sharing-leave", ButtonKind::Ghost, ctx.t("sharing.linked.leaveButton"))
            .icon(Icon::LogOut)
            .on_click(move || asking.set(true)),
        ),
      );
    }
    control = control.with_children(self.person.reason(ctx, LEAVE));
    if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
      control = control.child(field_error("sharing-leave-error", &reason.text(ctx)));
    }
    let leave = side_row(
      Label::new(
        "sharing-leave-row",
        ctx.t_args("sharing.linked.leave", [("project", shared)]),
      )
      .description(ctx.t_args("sharing.linked.leaveDescription", [("server", server)])),
      control,
      Place::Last,
      true,
    );
    section(
      "project-settings-sharing-leave",
      &ctx.t("sharing.linked.leaveSection"),
      None,
      vec![make_local, leave],
    )
    .into()
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
    Callout::new(
      "sharing-leave-question",
      CalloutTone::Warning,
      ctx.t_args("sharing.linked.leaveQuestion", [("project", name)]),
    )
    .icon(Icon::LogOut)
    .body(ctx.t_args("sharing.linked.leaveBody", [("project", name), ("server", server)]))
    .action(confirm)
    .action(
      ActionButton::new(
        "sharing-leave-keep",
        ButtonKind::Secondary,
        ctx.t("sharing.linked.leaveKeep"),
      )
      .on_click(move || keep.set(false)),
    )
    .into()
  }
}

/// *Shared on Kontur team* with the key, when it was linked and the person's role there.
fn shared_state(ctx: &Ctx, link: &orchester_store::SharingLink, (server, role): (&str, &str)) -> Column {
  Column::new()
    .width(Dimension::Px(CONTROL_WIDTH))
    .spacing(crate::theme::ExtraSpacing::Hair.role())
    .child(page::icon_note(
      "sharing-shared",
      Icon::Cloud,
      &ctx.t_args("sharing.linked.sharedOn", [("server", server)]),
    ))
    .child(page::note(
      "sharing-shared-detail",
      &ctx.t_args(
        "sharing.linked.detail",
        [
          ("key", link.key.as_str().to_owned()),
          ("day", crate::screens::settings::orchester::day_of(ctx, link.joined_at)),
          ("role", role.to_owned()),
          (
            "synced",
            crate::screens::settings::orchester::moment(ctx, link.last_synced_at),
          ),
        ],
      ),
    ))
}

/// *Members*, read-only, with who the person is there.
fn members_section(ctx: &mut Ctx, props: &Props, shared: &str, server: &str) -> Element {
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
      owner: link.is_some_and(|link| link.role == Some(MemberRole::Owner)),
      admin: account.is_some_and(|account| account.admin),
      me: account.map(|account| account.user),
      connected: entry
        .is_some_and(|entry| entry.connection == Some(orchester_workspace::sharing::Connection::Connected)),
      removed: link.and_then(|link| link.removed_at),
      username: account.map(|account| account.username.clone()),
    },
  )
}

/// The repository hint as the server says it; while it doesn't (offline, or no longer listing the project for a
/// removed member), the hint as last read stands, with when it was checked.
fn shown_hint(
  ctx: &mut Ctx,
  (props, link, entry): (
    &Props,
    &orchester_store::SharingLink,
    Option<&orchester_workspace::sharing::ServerEntry>,
  ),
  name: &str,
) -> Option<Result<rows::Hint, Said>> {
  match (rows::hint(ctx, link, entry), props.facts.cached_hint.as_ref()) {
    (Some(Err(_)), Some((cached, checked))) => Some(Ok(rows::Hint::cached(name.to_owned(), cached.clone(), *checked))),
    (live, _) => live,
  }
}
