//! The rows of a linked project's *Task list* section (`L5o9Xy`): *Project address*, read-only with the server's state
//! under it, and *Repository*, the project's repository hint with how this checkout's `origin` compares
//! (`Linked anyway · the mismatch stays visible` in `N9NFio`). The hint is read from the server while it is connected.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
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
    project_settings::page::CONTROL_WIDTH,
    settings::orchester::{Said, moment, sharing_said},
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::FieldInput,
    form_section::{Label, Place, side_row},
    icon::{Icon, icon},
  },
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

/// *Project address*: the address in a read-only field, and the server's name, state and certificate under it.
pub(super) fn address_row(ctx: &mut Ctx, link: &SharingLink, entry: Option<&ServerEntry>) -> Element {
  let address = link
    .address
    .clone()
    .unwrap_or_else(|| ctx.t("sharing.linked.noAddress").to_string());
  let field = FieldInput::new(
    "sharing-address-value",
    ctx.t("sharing.linked.address"),
    Signal::new(address),
  )
  .mono()
  .locked()
  .render(ctx);
  let state = entry.map_or_else(
    || ctx.t("sharing.linked.unknownServer").to_string(),
    |entry| server_state(ctx, entry),
  );
  let control = Column::new()
    .width(Dimension::Px(CONTROL_WIDTH))
    .spacing(ExtraSpacing::Snug.role())
    .child(field)
    .child(read_back(
      "sharing-address-state",
      (Icon::Server, PaletteColor::TextMuted),
      &state,
    ));
  side_row(
    Label::new("sharing-address", ctx.t("sharing.linked.address"))
      .description(ctx.t("sharing.linked.addressDescription")),
    control,
    Place::Within,
    true,
  )
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

/// *Repository*: the project's hint in a read-only field, and how this checkout's `origin` compares with it.
pub(super) fn repository_row(
  ctx: &mut Ctx,
  props: &Props,
  hint: Option<&Result<Hint, Said>>,
  names: (&str, &str),
) -> Element {
  let (project, server) = names;
  let value = match hint {
    Some(Ok(Hint { hint: Some(hint), .. })) => hint.as_str().to_owned(),
    Some(Ok(Hint { hint: None, .. })) => ctx.t("sharing.linked.repository.noHint").to_string(),
    Some(Err(_)) | None => String::new(),
  };
  let field = FieldInput::new(
    "sharing-repository-value",
    ctx.t("sharing.link.repository"),
    Signal::new(value),
  )
  .mono()
  .locked()
  .placeholder(ctx.t("sharing.linked.repository.reading"))
  .render(ctx);
  let (glyph, color, words) = compared(ctx, props, hint);
  let control = Column::new()
    .width(Dimension::Px(CONTROL_WIDTH))
    .spacing(ExtraSpacing::Snug.role())
    .child(field)
    .child(read_back("sharing-repository-state", (glyph, color), &words));
  side_row(
    Label::new("sharing-repository", ctx.t("sharing.link.repository")).description(ctx.t_args(
      "sharing.linked.repository.description",
      [("project", project), ("server", server)],
    )),
    control,
    Place::Within,
    true,
  )
}

/// The read-back under the repository's field.
fn compared(ctx: &Ctx, props: &Props, hint: Option<&Result<Hint, Said>>) -> (Icon, PaletteColor, String) {
  let muted = |words: String| (Icon::Info, PaletteColor::TextMuted, words);
  let hint = match hint {
    None => return muted(ctx.t("sharing.linked.repository.reading").to_string()),
    Some(Err(reason)) => return muted(reason.text(ctx)),
    Some(Ok(hint)) => hint,
  };
  let origin = props.facts.origin.as_ref();
  match remote_match(origin, hint.hint.as_ref()) {
    RemoteMatch::Same => (
      Icon::CircleCheck,
      PaletteColor::Success,
      ctx
        .t_args("sharing.linked.repository.same", [("time", moment(ctx, hint.checked))])
        .to_string(),
    ),
    RemoteMatch::Different { checkout, .. } => (
      Icon::TriangleAlert,
      PaletteColor::Warning,
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
    RemoteMatch::NoOrigin => (
      Icon::GitBranch,
      PaletteColor::TextMuted,
      ctx.t("sharing.linked.repository.noOrigin").to_string(),
    ),
    RemoteMatch::NoHint => muted(ctx.t("sharing.linked.repository.nothingToCompare").to_string()),
  }
}

/// A line under a row's control: a 12 glyph and the words, 5 apart, in `color` (`Read-back` in `L5o9Xy`).
fn read_back(id: &str, (glyph, color): (Icon, PaletteColor), words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Sm, color.clone()))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Note.role())
        .color(color)
        .flex(1.0),
    )
}
