//! A linked project's Sharing (`L5o9Xy`, *Removed from the project* in `N9NFio`): where its task list lives, its
//! address with the server's state, and *Leave or make local*. *Leave* asks in place and keeps a local copy of the
//! tasks and this computer's connection to the server; *Make local* is N2's and says so. Once the person is no longer a
//! member, the page says so instead of offering *Leave*.

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

mod rows;

use super::Facts;
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
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.facts == other.facts
  }
}

impl DevtoolsInspectable for Props {}

pub(super) struct Linked {
  asking: Signal<bool>,
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
    let hint = rows::hint(ctx, link, entry);
    // The server project's name as the server answered; this project's own until it has (`Kontur` in both, in the
    // design).
    let shared = match &hint {
      Some(Ok(hint)) => hint.name().to_owned(),
      _ => name.clone(),
    };
    let state = shared_state(ctx, link, (&server, &role));
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
    let mut column = Column::new()
      .id("project-settings-sharing")
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::ExtraSpacing::Roomy.role())
      .child(section(
        "project-settings-sharing-task-list",
        &ctx.t("sharing.link.taskList"),
        None,
        vec![task_list, address_row, repository_row],
      ));
    if link.removed_at.is_some() {
      column = column.child(
        Callout::new(
          "sharing-removed",
          CalloutTone::Warning,
          ctx.t_args("sharing.linked.removed.title", [("project", shared.as_str())]),
        )
        .icon(Icon::UserX)
        .body(ctx.t_args("sharing.linked.removed.body", [("project", shared.as_str())])),
      );
    }
    column.child(self.leave_section(ctx, &props, (&name, &shared, &server), link.key.as_str()))
  }
}

impl Linked {
  /// *Leave or make local*: *Make local* (N2, not in this version) and *Leave Kontur*, asked in place.
  /// `name` is this project's, `shared` the server project's.
  fn leave_section(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (name, shared, server): (&str, &str, &str),
    key: &str,
  ) -> Element {
    let make_local = side_row(
      Label::new("sharing-make-local", ctx.t("sharing.linked.makeLocal"))
        .description(ctx.t_args("sharing.linked.makeLocalDescription", [("project", name), ("key", key)])),
      Column::new().width(Dimension::Px(CONTROL_WIDTH)).child(
        ActionButton::new(
          "sharing-make-local-button",
          ButtonKind::Secondary,
          ctx.t("sharing.linked.makeLocalButton"),
        )
        .icon(Icon::Unlink)
        .unavailable(ctx.t("sharing.linked.makeLocalNotBuilt")),
      ),
      Place::Within,
      true,
    );
    let removed = props.facts.link.as_ref().is_some_and(|link| link.removed_at.is_some());
    let state = self.leave.state().get();
    let mut control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm);
    if removed {
      control = control.child(page::note("sharing-left", &ctx.t("sharing.linked.left")));
    } else if self.asking.get() {
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
