//! The rows of a linked project's *Team server* group (`Team server` `rsQ79` in `04b Sharing · shared` `TnUGj`):
//! *Address*, read-only with when it last synced and the server's state, *Your role*, and *Repository*, the project's
//! repository hint with how this checkout's `origin` compares (`Linked anyway · the mismatch stays visible` in
//! `N9NFio`). The hint is read from the server while it is connected.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  node::Element,
};
use orchester_core::{repository::RepositoryIdentity, time::Timestamp};
use orchester_store::SharingLink;
use orchester_workspace::{
  background,
  sharing::{Connection, RemoteMatch, ServerEntry, remote_match},
};

use super::Props;
use crate::{
  screens::{
    project_settings::kit::{SettingRow, Tone, value},
    settings::orchester::{Said, moment, sharing_said, shown_address},
  },
  services::Services,
};

/// The linked project's repository hint as the server answered, and when.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Hint {
  name: String,
  hint: Option<RepositoryIdentity>,
  checked: Timestamp,
}

impl DevtoolsInspectable for Hint {}

impl Hint {
  /// The hint as last read at `checked`, for while the server doesn't say; `name` is this project's.
  pub(super) const fn cached(name: String, hint: Option<RepositoryIdentity>, checked: Timestamp) -> Self {
    Self { name, hint, checked }
  }

  /// The server project's name.
  pub(super) fn name(&self) -> &str {
    &self.name
  }
}

/// The hint, read while `entry` is connected; why it is not known otherwise.
pub(super) fn hint(ctx: &mut Ctx, link: &SharingLink, entry: Option<&ServerEntry>) -> Option<Result<Hint, Said>> {
  let Some(entry) = entry else {
    return Some(Err(Said::new("sharing.linked.repository.unknownServer")));
  };
  match entry.connection {
    Some(Connection::Connected) => {}
    Some(Connection::Checking) => return None,
    _ => {
      return Some(Err(
        Said::new("sharing.linked.repository.unread").with("server", entry.stored.name.clone()),
      ));
    }
  }
  let servers = Services::of(ctx).servers;
  let state = ctx
    .future(
      (entry.stored.id, link.project, servers),
      |(server, project, servers)| async move {
        // Looked up here, so the read is not repeated whenever the entry's last answer changes.
        let entries = servers.clone();
        let entry = background::run(move || entries.entries())
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| sharing_said(&error))?
          .into_iter()
          .find(|entry| entry.stored.id == server)
          .ok_or_else(|| Said::new("sharing.linked.repository.unknownServer"))?;
        let projects = servers
          .server_projects(&entry)
          .await
          .map_err(|error| sharing_said(&error))?;
        let found = projects
          .into_iter()
          .find(|listed| listed.project.id == project)
          .ok_or_else(|| Said::new("sharing.linked.repository.notListed"))?;
        Ok(Hint {
          name: found.project.name,
          hint: found.project.remote_hint,
          checked: Timestamp::now(),
        })
      },
    )
    .state()
    .get();
  match (state.data, state.error) {
    (_, Some(error)) => Some(Err(error)),
    (Some(hint), _) => Some(Ok(hint)),
    _ => None,
  }
}

/// *Address* (`aou4M`): the address, *Synced 16:40* and the server's name, state and certificate; `server` names it
/// once it left the list.
pub(super) fn address_row(ctx: &mut Ctx, link: &SharingLink, (entry, server): (Option<&ServerEntry>, &str)) -> Element {
  let address = link.address.as_deref().map_or_else(
    || ctx.t("sharing.linked.noAddress").to_string(),
    |address| shown_address(address).to_owned(),
  );
  let state = entry.map_or_else(
    || {
      ctx
        .t_args("sharing.linked.missingServer", [("server", server)])
        .to_string()
    },
    |entry| server_state(ctx, entry),
  );
  let hint = ctx.t_args(
    "sharing.team.addressHint",
    [("time", moment(ctx, link.last_synced_at)), ("state", state)],
  );
  SettingRow::new("sharing-address", ctx.t("sharing.team.address"))
    .hint(hint)
    .info(ctx.t("sharing.linked.addressDescription"))
    .control(value("sharing-address-value", &address))
    .render(ctx)
}

/// *Your role* (`Z1IkQX`): owner or member, and since when.
pub(super) fn role_row(ctx: &mut Ctx, link: &SharingLink, role: &str) -> Element {
  let since = ctx.t_args(
    "sharing.team.roleHint",
    [
      ("key", link.key.as_str().to_owned()),
      ("day", crate::screens::settings::orchester::day_of(ctx, link.joined_at)),
    ],
  );
  SettingRow::new("sharing-role", ctx.t("sharing.team.role"))
    .hint(since)
    .control(value("sharing-role-value", role))
    .render(ctx)
}

/// `Kontur team · connected · certificate checked`.
fn server_state(ctx: &Ctx, entry: &ServerEntry) -> String {
  let state = ctx.t(if entry.certificate_changed {
    "sharing.linked.state.certificateChanged"
  } else {
    match entry.connection {
      Some(Connection::Connected) => "sharing.linked.state.connected",
      Some(Connection::Checking | Connection::Reconnecting { .. }) => "sharing.linked.state.connecting",
      Some(Connection::Offline { .. }) => "sharing.linked.state.offline",
      Some(Connection::SignedOut) | None => "sharing.linked.state.signedOut",
    }
  });
  let certificate = ctx.t(if entry.stored.certificate_pin.is_some() {
    "sharing.linked.certificate.checked"
  } else if entry.address.is_https() {
    "sharing.linked.certificate.system"
  } else {
    "sharing.linked.certificate.plain"
  });
  ctx
    .t_args(
      "sharing.linked.serverState",
      [
        ("server", entry.stored.name.clone()),
        ("state", state.to_string()),
        ("certificate", certificate.to_string()),
      ],
    )
    .to_string()
}

/// *Repository*: the project's hint, and how this checkout's `origin` compares with it in its tone.
pub(super) fn repository_row(
  ctx: &mut Ctx,
  props: &Props,
  hint: Option<&Result<Hint, Said>>,
  names: (&str, &str),
) -> Element {
  let (project, server) = names;
  let shown = match hint {
    Some(Ok(Hint { hint: Some(hint), .. })) => hint.as_str().to_owned(),
    Some(Ok(Hint { hint: None, .. })) => ctx.t("sharing.linked.repository.noHint").to_string(),
    Some(Err(_)) | None => ctx.t("sharing.linked.repository.reading").to_string(),
  };
  let (tone, words) = compared(ctx, props, hint);
  SettingRow::new("sharing-repository", ctx.t("sharing.link.repository"))
    .toned(words, tone)
    .info(ctx.t_args(
      "sharing.linked.repository.description",
      [("project", project), ("server", server)],
    ))
    .control(value("sharing-repository-value", &shown))
    .render(ctx)
}

/// How the checkout's `origin` compares with the hint, in its tone.
fn compared(ctx: &Ctx, props: &Props, hint: Option<&Result<Hint, Said>>) -> (Tone, String) {
  let hint = match hint {
    None => return (Tone::Muted, ctx.t("sharing.linked.repository.reading").to_string()),
    Some(Err(reason)) => return (Tone::Muted, reason.text(ctx)),
    Some(Ok(hint)) => hint,
  };
  let origin = props.facts.origin.as_ref();
  match remote_match(origin, hint.hint.as_ref()) {
    RemoteMatch::Same => (
      Tone::Success,
      ctx
        .t_args("sharing.linked.repository.same", [("time", moment(ctx, hint.checked))])
        .to_string(),
    ),
    RemoteMatch::Different { checkout, .. } => (
      Tone::Warning,
      ctx
        .t_args(
          "sharing.linked.repository.different",
          [
            ("origin", checkout.as_str().to_owned()),
            ("time", moment(ctx, hint.checked)),
          ],
        )
        .to_string(),
    ),
    RemoteMatch::NoOrigin => (Tone::Muted, ctx.t("sharing.linked.repository.noOrigin").to_string()),
    RemoteMatch::NoHint => (
      Tone::Muted,
      ctx.t("sharing.linked.repository.nothingToCompare").to_string(),
    ),
  }
}
