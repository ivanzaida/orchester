//! A linked project's task list says where it lives (`Shared line` in `Xr4Wb`, its states in `aocsw`): one quiet line
//! under the description with the server (to its entry in Settings › Orchester), the project's address (left out
//! below 1280), how the list stands with the server, the person's role there and *Sharing settings*. A state that
//! needs an action carries it: *Try now* while offline, *Join again* once signed out, *Check the new code* when the
//! certificate changed. Once the person is no longer a member the list is read-only and a notice over it says why,
//! with *Check again* and *Make local…* (`sErKV`).

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{Breakpoint, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::MemberRole;
use orchester_core::id::ProjectId;
use orchester_store::ProjectDb;
use orchester_workspace::{
  background,
  sharing::{
    Connection,
    project::state::{ProjectSharing, TaskListAccess},
  },
};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchesterView, ProjectPage, ProjectSettingsTab, Route},
  screens::settings::orchester::{follow, follow_shared, moment, shown_address},
  services::Services,
  theme::{ExtraColor, ExtraRadius, IconSize, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    pressable::opening,
  },
};

/// The sync state's dot (`Dot` `cJHCe`).
const DOT: f32 = 6.0;

/// How the project's task list stands with its server, and the owners as last read, whom a removed person asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Shared {
  pub sharing: ProjectSharing,
  pub owners: Vec<String>,
  pub tasks: usize,
}

impl DevtoolsInspectable for Shared {}

impl Shared {
  /// Why nothing can be added, once the person is no longer a member.
  pub fn read_only(&self, ctx: &Ctx) -> Option<Arc<str>> {
    matches!(self.sharing.access(), TaskListAccess::NotMember { .. }).then(|| ctx.t("tasks.shared.readOnly"))
  }
}

/// The project's sharing as it stands, read again whenever a server answers; `None` while the project is local.
pub(super) fn read(ctx: &mut Ctx, project: &ShownProject) -> Option<Shared> {
  let services = Services::of(ctx);
  let revision = follow(ctx, (&services.servers, &services.cloud)) + follow_shared(ctx, &services.servers);
  let root = project.open.repo_root.clone();
  ctx
    .future((revision, root, services.servers), |(_, root, servers)| async move {
      let read = background::run(
        move || -> Result<Option<Shared>, orchester_workspace::sharing::SharingError> {
          let Some(sharing) = servers.project_sharing(&root)? else {
            return Ok(None);
          };
          let db = ProjectDb::open(&root)?;
          let owners = db
            .shared_members()?
            .map(|(_, members)| {
              members
                .into_iter()
                .filter(|member| member.role == MemberRole::Owner)
                .map(|member| member.display_name)
                .collect()
            })
            .unwrap_or_default();
          let tasks = orchester_core::store::TaskStore::tasks(&db)?.len();
          Ok(Some(Shared { sharing, owners, tasks }))
        },
      )
      .await;
      match read {
        Ok(Ok(shared)) => Ok(shared),
        // The list is read through the project feed, which says why it cannot be; this line is left out then.
        Ok(Err(error)) => {
          tracing::warn!(%error, "the task list's sharing state cannot be read");
          Err(())
        }
        Err(_) => Err(()),
      }
    })
    .state()
    .get()
    .data
    .flatten()
}

/// The line under the task list's description.
pub(super) fn line(ctx: &mut Ctx, project: ProjectId, shared: &Shared) -> Row {
  let sharing = &shared.sharing;
  let navigation = Navigation::of(ctx);
  let server_view = OrchesterView::server(sharing.link.server);
  let server = opening(navigation.to(Route::Orchester(server_view)))
    .id("tasks-shared-server")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Cloud, IconSize::Base, PaletteColor::TextSecondary))
    .child(
      Text::new(&ctx.t_args("tasks.shared.on", [("server", sharing.server_name())]))
        .variant(TypographyStyle::Label)
        .nowrap(),
    );
  let mut row = Row::new()
    .id("tasks-shared-line")
    .padding(Padding::new().top(lurq::app::theme::SpacingSize::Xs))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(server);
  let wide = ctx.breakpoint() >= Some(Breakpoint::Xl);
  if let Some(address) = sharing.link.address.as_deref().filter(|_| wide) {
    row = row.child(separator(ctx)).child(
      Text::new(shown_address(address))
        .id("tasks-shared-address")
        .variant(OrchesterText::Address.role())
        .nowrap(),
    );
  }
  let settings = opening(navigation.to(Route::Project(
    project,
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  )))
  .id("tasks-shared-settings")
  .spacing(SpacingSize::Xs)
  .align_items(Alignment::Center)
  .child(
    Text::new(&ctx.t("tasks.shared.settings"))
      .variant(TypographyStyle::Description)
      .color(ExtraColor::AccentText.role())
      .nowrap(),
  )
  .child(icon(Icon::ArrowUpRight, IconSize::Sm, ExtraColor::AccentText.role()));
  row
    .child(separator(ctx))
    .child(state(ctx, sharing))
    .child(separator(ctx))
    .child(role(ctx, sharing))
    .child(separator(ctx))
    .child(settings)
}

fn separator(ctx: &Ctx) -> Text {
  Text::new(&ctx.t("tasks.shared.separator"))
    .variant(TypographyStyle::Description)
    .color(PaletteColor::TextMuted)
}

/// How the list stands with the server, with the one action that state needs.
fn state(ctx: &mut Ctx, sharing: &ProjectSharing) -> Row {
  let row = Row::new()
    .id("tasks-shared-state")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center);
  let server = sharing.link.server;
  if let Some(since) = sharing.link.removed_at {
    return row
      .child(icon(Icon::UserX, IconSize::Base, PaletteColor::Warning))
      .child(label(
        &ctx.t_args("tasks.shared.notMember", [("time", moment(ctx, since))]),
        PaletteColor::Warning,
      ));
  }
  if sharing.certificate_changed {
    return row
      .child(icon(Icon::ShieldAlert, IconSize::Base, PaletteColor::Danger))
      .child(label(&ctx.t("tasks.shared.certificateChanged"), PaletteColor::Danger))
      .child(action(ctx, "tasks-shared-check-code", "tasks.shared.checkCode", server));
  }
  match sharing.connection {
    Some(Connection::Connected | Connection::Checking) => row.child(dot(PaletteColor::Success)).child(label(
      &ctx.t_args(
        "tasks.shared.synced",
        [("time", moment(ctx, sharing.link.last_synced_at))],
      ),
      PaletteColor::TextSecondary,
    )),
    Some(Connection::Reconnecting { attempt, .. }) => row.child(dot(PaletteColor::TextMuted)).child(label(
      &ctx.t_args("tasks.shared.reconnecting", [("attempt", attempt.to_string())]),
      PaletteColor::TextMuted,
    )),
    Some(Connection::Offline { since, .. }) => {
      let servers = Services::of(ctx).servers;
      let words = ctx.t_args("tasks.shared.offline", [("time", moment(ctx, since))]);
      row
        .child(icon(Icon::CloudOff, IconSize::Base, PaletteColor::Warning))
        .child(label(&words, PaletteColor::Warning))
        .child(
          ActionButton::new("tasks-shared-try-now", ButtonKind::Ghost, ctx.t("tasks.shared.tryNow"))
            .compact()
            .on_click(move || servers.retry_now(server)),
        )
    }
    Some(Connection::SignedOut) | None => row
      .child(icon(Icon::LogOut, IconSize::Base, PaletteColor::Warning))
      .child(label(&ctx.t("tasks.shared.signedOut"), PaletteColor::Warning))
      .child(action(ctx, "tasks-shared-join-again", "tasks.shared.joinAgain", server)),
  }
}

/// An action that opens the server's entry in Settings › Orchester.
fn action(ctx: &mut Ctx, id: &str, key: &str, server: orchester_core::id::ServerId) -> ActionButton {
  let navigation = Navigation::of(ctx);
  ActionButton::new(id.to_owned(), ButtonKind::Ghost, ctx.t(key))
    .compact()
    .opens(navigation.to(Route::Orchester(OrchesterView::server(server))))
}

fn label(words: &str, color: PaletteColor) -> Text {
  Text::new(words)
    .variant(TypographyStyle::Description)
    .color(color)
    .nowrap()
}

fn dot(color: PaletteColor) -> Row {
  Row::new()
    .width(Dimension::Px(DOT))
    .height(Dimension::Px(DOT))
    .rounded(ExtraRadius::Full.role())
    .background(color)
}

/// *You're a member · oleh*, as the server gives the role; without the username once this computer signed out of the
/// server, which forgets the account.
fn role(ctx: &Ctx, sharing: &ProjectSharing) -> Row {
  let key = match (sharing.link.removed_at, sharing.link.role) {
    (Some(_), _) => "tasks.shared.wasMember",
    (None, Some(MemberRole::Owner)) => "tasks.shared.owner",
    (None, Some(MemberRole::Member)) => "tasks.shared.member",
    (None, None) => "tasks.shared.admin",
  };
  Row::new()
    .id("tasks-shared-role")
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::User, IconSize::Base, PaletteColor::TextMuted))
    .child(label(
      &match sharing.username.as_deref() {
        Some(username) => ctx.t_args(
          "tasks.shared.withName",
          [("role", ctx.t(key).to_string()), ("username", username.to_owned())],
        ),
        None => ctx.t(key),
      },
      PaletteColor::TextSecondary,
    ))
}

/// *You're no longer a member of Kontur on Kontur team* with *Check again* and *Make local…*; `None` while the person
/// is one.
pub(super) fn not_member(ctx: &mut Ctx, project: &ShownProject, shared: &Shared) -> Option<Element> {
  let sharing = &shared.sharing;
  let since = sharing.link.removed_at?;
  let separator = ctx.t("tasks.shared.or");
  let local = project.open.project.name().as_str().to_owned();
  let owners = if shared.owners.is_empty() {
    ctx.t("tasks.shared.anOwner").to_string()
  } else {
    shared.owners.join(&separator)
  };
  let body = ctx.t_args(
    "tasks.shared.removedBody",
    [
      ("server", sharing.server_name().to_owned()),
      ("time", moment(ctx, since)),
      ("project", local.clone()),
      ("key", sharing.link.key.as_str().to_owned()),
      ("count", shared.tasks.to_string()),
      ("owners", owners),
    ],
  );
  let servers = Services::of(ctx).servers;
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  Some(
    Notice::new(
      "tasks-not-member",
      NoticeTone::Warning,
      ctx.t_args(
        "tasks.shared.removedTitle",
        [("project", local.as_str()), ("server", sharing.server_name())],
      ),
    )
    .icon(Icon::UserX)
    .body(body)
    .action(
      ActionButton::new(
        "tasks-not-member-check",
        ButtonKind::Secondary,
        ctx.t("tasks.shared.checkAgain"),
      )
      .icon(Icon::RefreshCw)
      .on_click(move || servers.sync_now()),
    )
    .action(
      ActionButton::new(
        "tasks-not-member-make-local",
        ButtonKind::Primary,
        ctx.t("tasks.shared.makeLocal"),
      )
      .icon(Icon::Unplug)
      .opens(navigation.to(Route::Project(id, ProjectPage::MakeLocal))),
    )
    .into(),
  )
}
