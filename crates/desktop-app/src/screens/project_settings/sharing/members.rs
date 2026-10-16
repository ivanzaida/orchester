//! *Members*, read-only (`Section · Members` in `L5o9Xy`; *Members · as a member* and *Members · server offline* in
//! `N9NFio`; `docs/architecture/server.md`, row 63): who is in the project and their roles, as the server lists them
//! now or as last read while it cannot be reached, once this computer is signed out of it or it left the list, and once
//! the person is no longer a member. Owners and the server's admins get *Manage members and invites*, which opens the
//! project's page in the server admin in the browser, only on the server the project's address names; a member is
//! told to ask an owner. The app has no member or role control.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
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
    project_settings::page,
    settings::orchester::{Said, day_of, moment, sharing_said, shown_address},
  },
  services::Services,
  theme::{ExtraTypography, OrchesterText},
  ui::{
    avatar::{initials, person_avatar},
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::section,
    icon::Icon,
  },
};

/// The member table's columns (`Member header` `GFbX0`): the name takes the rest, the role and when they were added
/// have their own.
const ROLE_COLUMN: f32 = 90.0;
const ADDED_COLUMN: f32 = 80.0;
/// Where the member controls stood (`Actions` `s8ykaN`): the server admin has them now, and the column stays empty so
/// the table keeps the design's columns.
const ACTIONS_COLUMN: f32 = 220.0;
/// A member's row and the header (`Member row` `f2kjDI`).
const ROW_HEIGHT: f32 = 44.0;
const HEADER_HEIGHT: f32 = 30.0;

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
    let (rows, count) = match (&state.data, &state.error) {
      (_, Some(reason)) => (vec![Self::unread(ctx, reason)], None),
      (Some(Read(members)), None) => (self.read(ctx, &props, members), Some(members.members.len())),
      (None, None) => (
        vec![page::note("sharing-members-reading", &ctx.t("sharing.members.reading"))],
        None,
      ),
    };
    let count = count.map(|count| {
      let counted = match count {
        1 => ctx.t("sharing.members.countOne").to_string(),
        count => ctx
          .t_args("sharing.members.count", [("count", count.to_string())])
          .to_string(),
      };
      match &state.data {
        Some(Read(members)) if not_member(&props, members) => ctx
          .t_args(
            "sharing.members.countAsOf",
            [("count", counted), ("time", moment(ctx, members.read_at))],
          )
          .to_string(),
        _ => counted,
      }
    });
    section(
      "project-settings-sharing-members",
      &ctx.t("sharing.members.section"),
      count.as_deref(),
      rows,
    )
  }
}

impl Members {
  /// What the server said, or why the members cannot be shown.
  fn unread(ctx: &Ctx, reason: &Said) -> Element {
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::vertical(SpacingSize::Md))
      .child(field_error("sharing-members-error", &reason.text(ctx)))
      .into()
  }

  /// The intro with *Manage members and invites*, the table, and where Manage leads.
  fn read(&self, ctx: &Ctx, props: &Props, members: &ProjectMembers) -> Vec<Element> {
    let removed = not_member(props, members);
    let manages = (props.owner || props.admin) && !removed;
    let mut rows = vec![self.intro(ctx, props, members, manages), header(ctx).into()];
    let until = removed.then(|| moment(ctx, members.read_at));
    rows.extend(members.members.iter().map(|member| {
      let me = (props.me == Some(member.user)).then_some(until.as_deref());
      member_row(ctx, member, me).into()
    }));
    if removed {
      rows.push(removed_note(ctx, props, members));
    }
    if let Some(reason) = self.opened.get() {
      rows.push(field_error("sharing-members-open-failed", &reason.text(ctx)).into());
    }
    if manages && let Ok((_, url)) = admin_page(ctx, props) {
      rows.push(
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::new().top(SpacingSize::Md))
          .child(page::wide_note(
            "sharing-members-manage-note",
            Icon::ExternalLink,
            &ctx.t_args("sharing.members.manageNote", [("url", strip(&url))]),
          ))
          .into(),
      );
    }
    rows
  }

  fn intro(&self, ctx: &Ctx, props: &Props, members: &ProjectMembers, manages: bool) -> Element {
    let names = |key: &str| {
      let owners: Vec<&str> = members.owners().map(|owner| owner.display_name.as_str()).collect();
      let separator = ctx.t("sharing.members.or");
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
    };
    let text = match (members.read, manages) {
      _ if not_member(props, members) => names("sharing.members.introRemoved"),
      (MembersRead::Live, true) => names("sharing.members.intro"),
      (MembersRead::Live, false) => names("sharing.members.introMember"),
      (MembersRead::NoServer, _) => names("sharing.members.introNoServer"),
      (_, true) => names("sharing.members.introOffline"),
      (_, false) => names("sharing.members.introOfflineMember"),
    };
    let mut row = Row::new()
      .id("sharing-members-intro")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Md).bottom(SpacingSize::Md))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::Center)
      .child(
        Text::new(&text)
          .id("sharing-members-intro-text")
          .variant(ExtraTypography::Lead.role())
          .flex(1.0),
      );
    if manages {
      row = row.child(self.manage(ctx, props, members.read));
    }
    row.into()
  }

  /// *Manage members and invites*: the project's page in the server admin, in the browser; unavailable, with why, while
  /// the server cannot be reached or this computer is signed out of it, and when the project's address is not on it.
  fn manage(&self, ctx: &Ctx, props: &Props, read: MembersRead) -> ActionButton {
    let button = ActionButton::new(
      "sharing-members-manage",
      ButtonKind::Secondary,
      ctx.t("sharing.members.manage"),
    )
    .icon(Icon::ExternalLink);
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
    Err(SharingError::ForeignAddress) => Err(ctx.t_args(
      "sharing.members.manageForeign",
      [("address", shown_address(text)), server],
    )),
    Err(error) => {
      tracing::warn!(%error, "the project's address cannot be read");
      Err(ctx.t_args("sharing.members.manageNoAddress", [server]))
    }
  }
}

/// An address as the note shows it, without `https://`.
fn strip(url: &str) -> String {
  shown_address(url).to_owned()
}

/// NAME, ROLE and ADDED, in 10.5/500 `text_muted` tracked 0.4, over a 1 `border` line.
fn header(ctx: &Ctx) -> Row {
  let heading = |key: &str| {
    Text::new(&ctx.t(key))
      .variant(ExtraTypography::ColumnHeading.role())
      .nowrap()
  };
  line(Row::new().id("sharing-members-header"), HEADER_HEIGHT)
    .child(Row::new().flex(1.0).child(heading("sharing.members.name")))
    .child(
      Row::new()
        .width(Dimension::Px(ROLE_COLUMN))
        .child(heading("sharing.members.role")),
    )
    .child(
      Row::new()
        .width(Dimension::Px(ADDED_COLUMN))
        .child(heading("sharing.members.added")),
    )
    .child(Row::new().width(Dimension::Px(ACTIONS_COLUMN)))
}

/// How to come back once the person is no longer a member: ask an owner (`Note` in `xLeTJ`).
fn removed_note(ctx: &Ctx, props: &Props, members: &ProjectMembers) -> Element {
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
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Md))
    .child(page::wide_note("sharing-members-come-back", Icon::Info, &text))
    .into()
}

/// A member: the avatar, name and username (`· you` for the person, `· you, until 09:12` once they are no longer
/// one), the role, and the day they were added.
fn member_row(ctx: &Ctx, member: &SharedMember, me: Option<Option<&str>>) -> Row {
  let id = format!("sharing-member-{}", member.username);
  let who = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(person_avatar(&format!("{id}-avatar"), &initials(&member.display_name)))
    .child(
      Text::new(&member.display_name)
        .id(format!("{id}-name"))
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextPrimary)
        .nowrap(),
    )
    .child(
      Text::new(&member.username)
        .id(format!("{id}-username"))
        .variant(OrchesterText::Address.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .with_children(me.map(|until| {
      let you = match until {
        Some(time) => ctx.t_args("sharing.members.youUntil", [("time", time)]),
        None => ctx.t("sharing.members.you"),
      };
      Text::new(&you)
        .id(format!("{id}-you"))
        .variant(ExtraTypography::Note.role())
        .nowrap()
    }));
  let role = ctx.t(match member.role {
    MemberRole::Owner => "sharing.members.owner",
    MemberRole::Member => "sharing.members.member",
  });
  line(Row::new().id(id.clone()), ROW_HEIGHT)
    .child(who)
    .child(
      Row::new().width(Dimension::Px(ROLE_COLUMN)).child(
        Text::new(&role)
          .id(format!("{id}-role"))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextPrimary),
      ),
    )
    .child(
      Row::new().width(Dimension::Px(ADDED_COLUMN)).child(
        Text::new(&day_of(ctx, member.added_at))
          .id(format!("{id}-added"))
          .variant(ExtraTypography::Prose.role())
          .color(PaletteColor::TextMuted),
      ),
    )
    .child(Row::new().width(Dimension::Px(ACTIONS_COLUMN)))
}

/// A row of the table: its cells 12 apart, on a 1 `border` line.
fn line(row: Row, height: f32) -> Row {
  row
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(height))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
}
