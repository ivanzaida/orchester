//! *Discovery file* of a linked project (`Row · Discovery file` in `L5o9Xy`): whether `orchester.toml` at the
//! repository root names the project's address, when it was written and whether it is committed. Without one,
//! *Write orchester.toml* writes it, which only a person does: colleagues who open the checkout are offered the
//! address. A file that names anything else is left as it is. Orchester never commits it.

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
use orchester_workspace::{
  background,
  sharing::project::discovery::{self, DiscoveryState, LinkedDiscovery},
};

use crate::{
  project::ShownProject,
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::{PersonOnly, Said, day_of},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::{Label, Place, side_row},
    icon::Icon,
  },
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub address: Option<String>,
  pub discovery: Option<LinkedDiscovery>,
  pub revision: Signal<u64>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.address == other.address && self.discovery == other.discovery
  }
}

impl DevtoolsInspectable for Props {}

/// *Write orchester.toml*, which only a person presses.
const WRITE: &str = "sharing-discovery-write";

pub(super) struct DiscoveryRow {
  write: FutureAction<(), (), Said>,
  person: PersonOnly,
}

impl Component for DiscoveryRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let (root, address, revision) = (
      props.project.open.repo_root.clone(),
      props.address.clone(),
      props.revision,
    );
    let write = ctx.future_action(move |(): ()| {
      let (root, address, revision) = (root.clone(), address.clone(), revision.clone());
      async move {
        let address = address.ok_or_else(|| Said::new("sharing.discovery.noAddress"))?;
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
    let mut control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Xs);
    control = match &props.discovery {
      None => control.child(page::note(
        "sharing-discovery-no-address",
        &ctx.t("sharing.discovery.noAddress"),
      )),
      Some(found) => match found.state {
        DiscoveryState::Names => written(ctx, control, found),
        DiscoveryState::Other => control.child(page::icon_note(
          "sharing-discovery-other",
          Icon::FileText,
          &ctx.t("sharing.discovery.other"),
        )),
        DiscoveryState::Absent => self.absent(ctx, control),
      },
    };
    let state = self.write.state().get();
    if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
      control = control.child(field_error("sharing-discovery-error", &reason.text(ctx)));
    }
    side_row(
      Label::new("sharing-discovery", ctx.t("sharing.discovery.label"))
        .description(ctx.t_args("sharing.discovery.description", [("local", local.as_str())])),
      control,
      Place::Last,
      true,
    )
  }
}

impl DiscoveryRow {
  /// No file yet: *Write orchester.toml*.
  fn absent(&self, ctx: &Ctx, control: Column) -> Column {
    let button = ActionButton::new(WRITE, ButtonKind::Secondary, ctx.t("sharing.discovery.write")).icon(Icon::FileText);
    let button = if self.write.state().get().is_pending() {
      button.loading()
    } else {
      let (write, person) = (self.write.clone(), self.person.clone());
      button.on_click(move || {
        if person.allows(WRITE) {
          write.run(());
        }
      })
    };
    control
      .child(page::note(
        "sharing-discovery-absent",
        &ctx.t("sharing.discovery.absent"),
      ))
      .child(Row::new().child(button))
      .with_children(self.person.reason(ctx, WRITE))
  }
}

/// `orchester.toml · written Oct 2` and whether it is committed.
fn written(ctx: &Ctx, control: Column, found: &LinkedDiscovery) -> Column {
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
  control
    .child(page::icon_note("sharing-discovery-file", Icon::FileText, &file))
    .child(page::icon_note(
      "sharing-discovery-committed",
      Icon::GitCommitHorizontal,
      &committed,
    ))
}
