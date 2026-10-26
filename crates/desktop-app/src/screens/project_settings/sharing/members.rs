//! *Members*, read-only (`Members` `R370H0` in `04b Sharing · shared` `TnUGj`; *Members · as a member* and *Members ·
//! server offline* in `N9NFio`; `docs/architecture/server.md`, row 63): who is in the project and their roles, as the
//! server lists them now or as last read while it cannot be reached, once this computer is signed out of it or it left
//! the list, and once the person is no longer a member. Owners and the server's admins get *Manage*, which opens the
//! project's page in the server admin in the browser, only on the server the project's address names; a member is
//! told to ask an owner. The app has no member or role control.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_api::MemberRole;
use orchester_core::{id::UserId, time::Timestamp};
use orchester_store::SharedMember;
use orchester_workspace::sharing::{
  ProjectAddress, ServerEntry, SharingError,
  project::{MembersRead, ProjectMembers},
};

use crate::{
  project::ShownProject,
  screens::{
    project_settings::kit::{Group, SettingRow, Tone, value},
    settings::orchester::{Said, day_of, moment, sharing_said, shown_address},
  },
  services::Services,
  ui::{
    avatar::{initials, person_avatar},
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// The members the summary names before *+N* (`dana-k, ivan, olha +1`).
const NAMED: usize = 3;

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub server: String,
  /// The server project's name, its address and the person's role there.
  pub shared: String,
  pub address: Option<String>,
  /// The project's server while it is in the list: *Manage* opens its admin while this computer is signed in to it.
  pub entry: Option<ServerEntry>,
  pub owner: bool,
  pub admin: bool,
  pub me: Option<UserId>,
  /// Whether the server answers now; the members are read again when it changes.
  pub connected: bool,
  /// Since when the person is no longer a member: the list stays as last read.
  pub removed: Option<Timestamp>,
  /// The person's username there, which an owner adds again.
  pub username: Option<String>,
}

impl DevtoolsInspectable for Props {}

/// The members as read, for the future's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(ProjectMembers);

impl DevtoolsInspectable for Read {}

pub(super) struct Members {
  links: crate::services::links::Links,
  /// Why the admin did not open in the browser.
  opened: Signal<Option<Said>>,
}

impl Component for Members {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      links: Services::of(ctx).links,
      opened: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let servers = Services::of(ctx).servers;
    let state = ctx
      .future(
        (
          props.project.open.repo_root.clone(),
          servers,
          props.connected,
          props.server.clone(),
        ),
        |(root, servers, _, name)| async move {
          match servers.project_members(&root).await {
            Ok(members) => Ok(Read(members)),
            Err(SharingError::Offline) => Err(Said::new("sharing.members.unread").with("server", name)),
            Err(error) => Err(sharing_said(&error)),
          }
        },
      )
      .state()
      .get();
    let title = ctx.t("sharing.members.section");
    let rows = match (&state.data, &state.error) {
      (_, Some(reason)) => vec![
        SettingRow::new("sharing-members", title.clone())
          .toned(reason.text(ctx), Tone::Danger)
          .render(ctx),
      ],
      (Some(Read(members)), None) => self.read(ctx, &props, members),
      (None, None) => vec![
        SettingRow::new("sharing-members", title.clone())
          .hint(ctx.t("sharing.members.reading"))
          .render(ctx),
      ],
    };
    Group::new("project-settings-sharing-members", title).rows(rows)
  }
}

impl Members {
  /// The summary with *Manage*, a row for each member, and how to come back once the person is no longer one.
  fn read(&self, ctx: &mut Ctx, props: &Props, members: &ProjectMembers) -> Vec<Element> {
    let removed = not_member(props, members);
    let manages = (props.owner || props.admin) && !removed;
    let mut rows = vec![self.summary(ctx, props, members, manages)];
    let until = removed.then(|| moment(ctx, members.read_at));
    // Once this computer signed out of the server it no longer knows the person's account there; the list knows who
    // read it.
    let mine = props.me.or(members.me);
    for member in &members.members {
      let me = (mine == Some(member.user)).then_some(until.as_deref());
      rows.push(member_row(ctx, member, me));
    }
    if removed {
      rows.push(removed_row(ctx, props, members));
    }
    rows
  }

  /// *Members* with who they are (`dana-k, ivan, olha +1`), how many, and *Manage*; what the list is and where *Manage*
  /// leads in its (i).
  fn summary(&self, ctx: &mut Ctx, props: &Props, members: &ProjectMembers, manages: bool) -> Element {
    let mut info = intro(ctx, props, members, manages);
    if manages && let Ok((_, url)) = admin_page(ctx, props) {
      let note = ctx.t_args("sharing.members.manageNote", [("url", shown_address(&url).to_owned())]);
      info = format!("{info}\n\n{note}");
    }
    let usernames: Vec<&str> = members
      .members
      .iter()
      .take(NAMED)
      .map(|member| member.username.as_str())
      .collect();
    let mut named = usernames.join(", ");
    if members.members.len() > NAMED {
      named = ctx
        .t_args(
          "sharing.members.namedMore",
          [("names", named), ("more", (members.members.len() - NAMED).to_string())],
        )
        .to_string();
    }
    // A list that is not the server's answer now says when it was read, whatever kept the server from answering.
    if !members.live() || not_member(props, members) {
      named = ctx
        .t_args(
          "sharing.members.asOf",
          [("names", named), ("time", moment(ctx, members.read_at))],
        )
        .to_string();
    }
    let error = self
      .opened
      .get()
      .map(|reason| crate::ui::field::field_error("sharing-members-open-failed", &reason.text(ctx)).into());
    let row = SettingRow::new("sharing-members", ctx.t("sharing.members.section"))
      .hint(named)
      .info(info)
      .control(value("sharing-members-count", &members.members.len().to_string()))
      .under_all(error);
    let row = if manages {
      row.control(self.manage(ctx, props, members.read))
    } else {
      row
    };
    row.render(ctx)
  }

  /// *Manage*: the project's page in the server admin, in the browser; unavailable, with why, while the server cannot
  /// be reached or this computer is signed out of it, and when the project's address is not on it.
  fn manage(&self, ctx: &Ctx, props: &Props, read: MembersRead) -> ActionButton {
    let button = ActionButton::new(
      "sharing-members-manage",
      ButtonKind::Secondary,
      ctx.t("sharing.members.manage"),
    )
    .icon(Icon::ExternalLink)
    .in_form();
    let (entry, url) = match admin_page(ctx, props) {
      Ok(page) => page,
      Err(reason) => return button.unavailable(reason),
    };
    if read != MembersRead::Live {
      return button.unavailable(ctx.t_args("sharing.members.manageOffline", [("server", props.server.as_str())]));
    }
    let (links, opened) = (self.links.clone(), self.opened.clone());
    button.on_click(move || match links.open_server_page(&entry, &url) {
      Ok(()) => opened.set(None),
      Err(error) => {
        tracing::warn!(%error, "the server admin could not be opened");
        opened.set(Some(Said::new("sharing.members.openFailed").with("url", url.clone())));
      }
    })
  }
}

/// What the list is: whose it is, who manages it and whether it is the server's answer now.
fn intro(ctx: &Ctx, props: &Props, members: &ProjectMembers, manages: bool) -> String {
  let owners: Vec<&str> = members.owners().map(|owner| owner.display_name.as_str()).collect();
  let separator = ctx.t("sharing.members.or");
  let key = match (members.read, manages) {
    _ if not_member(props, members) => "sharing.members.introRemoved",
    (MembersRead::Live, true) => "sharing.members.intro",
    (MembersRead::Live, false) => "sharing.members.introMember",
    (MembersRead::NoServer, _) => "sharing.members.introNoServer",
    (_, true) => "sharing.members.introOffline",
    (_, false) => "sharing.members.introOfflineMember",
  };
  ctx
    .t_args(
      key,
      [
        ("project", props.shared.clone()),
        ("server", props.server.clone()),
        ("time", moment(ctx, members.read_at)),
        ("owners", owners.join(&separator)),
      ],
    )
    .to_string()
}

/// Whether the person is no longer a member: the link says so, or the server no longer shows them the members.
fn not_member(props: &Props, members: &ProjectMembers) -> bool {
  props.removed.is_some() || members.read == MembersRead::NotMember
}

/// The project's page in its server's admin, with the server; why not while this computer is not signed in to it,
/// until the project's address was read, and when the address is not on that server.
fn admin_page(ctx: &Ctx, props: &Props) -> Result<(ServerEntry, String), std::sync::Arc<str>> {
  let server = ("server", props.server.as_str());
  let entry = props
    .entry
    .clone()
    .filter(ServerEntry::in_use)
    .ok_or_else(|| ctx.t_args("sharing.members.manageNoServer", [server]))?;
  let text = props
    .address
    .as_deref()
    .ok_or_else(|| ctx.t_args("sharing.members.manageNoAddress", [server]))?;
  match ProjectAddress::reported(text, &entry.address) {
    Ok(address) => Ok((entry, address.admin_url())),
    Err(SharingError::ForeignAddress { own }) => Err(ctx.t_args(
      "sharing.members.manageForeign",
      [
        ("address", shown_address(text)),
        ("own", shown_address(own.as_str())),
        server,
      ],
    )),
    Err(error) => {
      tracing::warn!(%error, "the project's address cannot be read");
      Err(ctx.t_args("sharing.members.manageNoAddress", [server]))
    }
  }
}

/// How to come back once the person is no longer a member: ask an owner (`Note` in `xLeTJ`).
fn removed_row(ctx: &mut Ctx, props: &Props, members: &ProjectMembers) -> Element {
  let separator = ctx.t("sharing.members.or");
  let owners: Vec<&str> = members.owners().map(|owner| owner.display_name.as_str()).collect();
  let names = [("owners", owners.join(&separator)), ("server", props.server.clone())];
  // Once this computer signed out of the server, it no longer knows the person's username there.
  let text = match props.username.clone() {
    Some(username) => {
      let [owners, server] = names;
      ctx.t_args("sharing.members.comeBack", [owners, ("username", username), server])
    }
    None => ctx.t_args("sharing.members.comeBackNoName", names),
  };
  SettingRow::new("sharing-members-come-back", ctx.t("sharing.members.comeBackTitle"))
    .hint(text)
    .render(ctx)
}

/// A member (`sharing-member-<username>`): the avatar, name, username and the day they were added (`· you` for the
/// person, `· you, until 09:12` once they are no longer one), and the role.
fn member_row(ctx: &mut Ctx, member: &SharedMember, me: Option<Option<&str>>) -> Element {
  let id = format!("sharing-member-{}", member.username);
  let mut hint = ctx
    .t_args(
      "sharing.members.memberHint",
      [
        ("username", member.username.clone()),
        ("day", day_of(ctx, member.added_at)),
      ],
    )
    .to_string();
  if let Some(until) = me {
    let you = match until {
      Some(time) => ctx.t_args("sharing.members.youUntil", [("time", time)]),
      None => ctx.t("sharing.members.you"),
    };
    hint = format!("{hint} {you}");
  }
  let role = ctx.t(match member.role {
    MemberRole::Owner => "sharing.members.owner",
    MemberRole::Member => "sharing.members.member",
  });
  SettingRow::new(id.clone(), member.display_name.clone())
    .item()
    .lead(person_avatar(&format!("{id}-avatar"), &initials(&member.display_name)))
    .hint(hint)
    .control(value(&format!("{id}-role"), &role))
    .render(ctx)
}
