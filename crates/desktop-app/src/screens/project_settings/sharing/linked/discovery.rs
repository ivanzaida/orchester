//! *Discovery file* of a linked project (`Discovery file` `r5LUQz` in `04b Sharing · shared` `TnUGj`): whether
//! `orchester.toml` at the repository root names the project's address, when it was written and whether it is
//! committed. Without one, *Write orchester.toml* writes it, which only a person does: colleagues who open the checkout
//! are offered the address, so it is written only on the server that reported it, and the button is unavailable with
//! why while this computer can't check that (`Manage` in `q8DIyg`). A file that names anything else is left as it is.
//! Orchester never commits it.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_api::client::ServerAddress;
use orchester_workspace::{
  background,
  sharing::{
    ProjectAddress, SharingError,
    project::discovery::{self, DiscoveryState, LinkedDiscovery},
  },
};

use crate::{
  project::ShownProject,
  screens::{
    project_settings::kit::{Group, SettingRow},
    settings::orchester::{PersonOnly, Said, day_of, shown_address},
  },
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
  pub address: Option<String>,
  /// The project's server while it is in the list, its name and address: the project's address is written only when
  /// it is on that server.
  pub server: Option<(String, ServerAddress)>,
  pub discovery: Option<LinkedDiscovery>,
  pub revision: Signal<u64>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.address == other.address
      && self.server == other.server
      && self.discovery == other.discovery
  }
}

impl DevtoolsInspectable for Props {}

/// *Write orchester.toml*, which only a person presses.
const WRITE: &str = "sharing-discovery-write";

pub(super) struct DiscoveryRow {
  /// Writes the address it is given, checked against the project's server before the press.
  write: FutureAction<String, (), Said>,
  person: PersonOnly,
}

impl Component for DiscoveryRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let (root, revision) = (props.project.open.repo_root.clone(), props.revision);
    let write = ctx.future_action(move |address: String| {
      let (root, revision) = (root.clone(), revision.clone());
      async move {
        background::run(move || discovery::write(&root, &address))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map_err(|error| {
            tracing::warn!(%error, "orchester.toml was not written");
            Said::new("sharing.discovery.writeFailed")
          })?;
        revision.update(|count| *count += 1);
        Ok(())
      }
    });
    Self {
      write,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let local = props.project.open.project.name().as_str().to_owned();
    let row = SettingRow::new("sharing-discovery", discovery::FILE)
      .mono_label()
      .info(ctx.t_args("sharing.discovery.description", [("local", local.as_str())]));
    let row = match &props.discovery {
      None => row.hint(ctx.t("sharing.discovery.noAddress")),
      Some(found) => match found.state {
        DiscoveryState::Names => row.hint(written(ctx, found)),
        DiscoveryState::Other => row.hint(ctx.t("sharing.discovery.other")),
        DiscoveryState::Absent => {
          let row = row.hint(ctx.t("sharing.discovery.absent"));
          self.absent(ctx, row, &props)
        }
      },
    };
    let state = self.write.state().get();
    let error = state
      .error
      .clone()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("sharing-discovery-error", &reason.text(ctx)).into());
    let row = row.under_all(error);
    Group::new("project-settings-sharing-discovery", ctx.t("sharing.discovery.label"))
      .row(row.render(ctx))
  }
}

impl DiscoveryRow {
  /// No file yet: *Write orchester.toml*, unavailable with why while the address cannot be checked against the
  /// project's server, or is not on it.
  fn absent(&self, ctx: &mut Ctx, row: SettingRow, props: &Props) -> SettingRow {
    let button = ActionButton::new(WRITE, ButtonKind::Secondary, ctx.t("sharing.discovery.write"))
      .icon(Icon::FileText)
      .in_form();
    let (button, reason) = match writable(ctx, props) {
      Err(reason) => (button.unavailable(reason.clone()), Some(reason)),
      Ok(_) if self.write.state().get().is_pending() => (button.loading(), None),
      Ok(address) => {
        let (write, person) = (self.write.clone(), self.person.clone());
        let button = button.on_click(move || {
          if person.allows(WRITE) {
            write.run(address.clone());
          }
        });
        (button, None)
      }
    };
    let reasons: Vec<Element> = reason
      .map(|reason| reason_line("sharing-discovery-unavailable", &reason).into())
      .into_iter()
      .chain(self.person.reason(ctx, WRITE))
      .collect();
    row.control(button).under_all(reasons)
  }
}

/// The project's address to write, once it is on the project's server: colleagues who open the checkout trust it. Why
/// not otherwise, said before any press.
fn writable(ctx: &Ctx, props: &Props) -> Result<String, Arc<str>> {
  let address = props
    .address
    .as_deref()
    .ok_or_else(|| ctx.t("sharing.discovery.noAddress"))?;
  let (name, server) = props
    .server
    .as_ref()
    .ok_or_else(|| ctx.t("sharing.discovery.noServer"))?;
  match ProjectAddress::reported(address, server) {
    Ok(address) => Ok(address.to_text()),
    Err(SharingError::ForeignAddress { own }) => Err(ctx.t_args(
      "sharing.discovery.foreignAddress",
      [("server", name.as_str()), ("own", shown_address(own.as_str()))],
    )),
    Err(error) => {
      tracing::warn!(%error, "the project's address cannot be read");
      Err(ctx.t_args("sharing.discovery.invalidAddress", [("server", name.as_str())]))
    }
  }
}

/// `Written Oct 2 · committed Oct 3`.
fn written(ctx: &Ctx, found: &LinkedDiscovery) -> String {
  let file = found.written_at.map_or_else(
    || ctx.t("sharing.discovery.file").to_string(),
    |at| {
      ctx
        .t_args("sharing.discovery.written", [("day", day_of(ctx, at))])
        .to_string()
    },
  );
  let committed = match (found.committed, found.committed_at) {
    (Some(true), Some(at)) => ctx.t_args("sharing.discovery.committedOn", [("day", day_of(ctx, at))]),
    (Some(true), None) => ctx.t("sharing.discovery.committed"),
    (Some(false), _) => ctx.t("sharing.discovery.notCommitted"),
    (None, _) => ctx.t("sharing.discovery.commitUnknown"),
  };
  ctx
    .t_args(
      "sharing.discovery.state",
      [("file", file), ("committed", committed.to_string())],
    )
    .to_string()
}
