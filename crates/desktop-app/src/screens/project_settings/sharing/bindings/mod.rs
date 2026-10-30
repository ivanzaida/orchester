//! Sharing › On this computer (`04b` `e41po`, `07` `C8JAJ`, `07c` `A3rnh8`; decisions 8 and 11 of
//! `docs/design/shared-configuration.md`): what the team's versions need that is the person's, grouped as
//! Connections, MCP servers and Secrets, each with who needs it and how it stands here ([`rows`]). A connection is
//! bound by name or by role on its own and can be changed through the picker of the person's connections, with *Add an
//! account…*; one with none of its role offers *Add an account…*. An MCP server or a secret is bound only after the
//! person confirms it in a modal ([`confirm`]): no MCP tool and no synthetic input binds one. Shown in a shared
//! project, above *Team server*.

mod confirm;
mod rows;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_store::BoundReference;
use orchester_workspace::{background, sharing::sync::Here};

use crate::{
  project::ShownProject,
  screens::{
    hub::parts::note,
    project_settings::kit::Group,
    settings::orchester::{PersonOnly, Said, follow_shared},
    shared_config::bind_action,
  },
  services::Services,
  theme::{ExtraSpacing, ProjectSettingsText},
  ui::{field::field_error, icon::Icon},
};

/// A connection bound from a picker, which only a person chooses.
const PICKED: &str = "sharing-bindings-picked";

/// What this computer has of the project's references, as read.
#[derive(Clone, Debug, PartialEq)]
struct Read(Here);

impl DevtoolsInspectable for Read {}

/// An MCP server or a secret the person is asked to bind, unbound as it is now, with its name and who needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Ask {
  pub reference: BoundReference,
  pub name: String,
  pub needs: String,
}

impl DevtoolsInspectable for Ask {}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
}

impl DevtoolsInspectable for Props {}

pub(super) struct OnThisComputer {
  /// Counts the bindings made here, each of which reads the references again.
  done: Signal<u64>,
  bind: FutureAction<BoundReference, (), Said>,
  person: PersonOnly,
  asking: Signal<Option<Ask>>,
  /// The modal is open.
  open: Signal<bool>,
  /// The local MCP server or secret the modal has chosen, by id.
  chosen: Signal<String>,
}

impl Component for OnThisComputer {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let root = ctx.props::<Props>().project.open.repo_root.clone();
    let done = ctx.signal(0);
    Self {
      bind: bind_action(ctx, root, done.clone()),
      done,
      person: PersonOnly::new(ctx),
      asking: ctx.signal(None),
      open: ctx.signal(false),
      chosen: ctx.signal(String::new()),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let revision = follow_shared(ctx, &services.servers) + self.done.get();
    let state = ctx
      .future((props.project.open.repo_root, revision), |(root, _)| async move {
        background::run(move || Here::read(&root))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map(Read)
          .map_err(|error| crate::screens::settings::orchester::sharing_said(&error))
      })
      .state()
      .get();
    let column = Column::new().id("sharing-bindings").width(Dimension::Pct(100.0));
    let shown: Element = match (state.data, state.error) {
      (Some(Read(here)), _) if !here.references.is_empty() => self.section(ctx, &here),
      (None, Some(reason)) => column
        .child(field_error("sharing-bindings-unread", &reason.text(ctx)))
        .into(),
      _ => column.into(),
    };
    shown
  }
}

impl OnThisComputer {
  /// The lead, the three groups, the note, and the modal while it asks.
  fn section(&self, ctx: &mut Ctx, here: &Here) -> Element {
    let actions = rows::Actions {
      bind: self.bind.clone(),
      person: self.person.clone(),
      asking: self.asking.clone(),
      open: self.open.clone(),
      chosen: self.chosen.clone(),
      busy: self.bind.state().get().is_pending(),
    };
    let groups = [
      ("connections", "sharing.bindings.connections", rows::Kind::Connection),
      ("servers", "sharing.bindings.servers", rows::Kind::McpServer),
      ("secrets", "sharing.bindings.secrets", rows::Kind::Secret),
    ]
    .into_iter()
    .filter_map(|(id, title, kind)| {
      let rows = rows::rows(ctx, here, kind, &actions);
      (!rows.is_empty()).then(|| Element::from(Group::new(&format!("sharing-bindings-{id}"), ctx.t(title)).rows(rows)))
    })
    .collect::<Vec<_>>();
    let state = self.bind.state().get();
    let failed: Option<Element> = state
      .error
      .clone()
      .filter(|_| !state.is_pending())
      .map(|reason| field_error("sharing-bindings-error", &reason.text(ctx)).into());
    let section = Column::new()
      .id("sharing-bindings")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Md)
      .child(lead(ctx, here))
      .with_children(groups)
      .with_children(failed)
      .with_children(self.person.reason(ctx, PICKED));
    let asked = self.asking.get().map(|ask| confirm::dialog(ctx, here, &ask, &actions));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(section)
      .child(note(
        "sharing-bindings-note",
        Icon::Lock,
        &ctx.t("sharing.bindings.note"),
      ))
      .with_children(asked)
      .into()
  }
}

/// *On this computer · 7 references · 3 need you*, and what it is.
fn lead(ctx: &Ctx, here: &Here) -> Element {
  let count = [
    ("count", here.references.len().to_string()),
    ("need", here.needs_you().to_string()),
  ];
  let count = if here.needs_you() == 0 {
    ctx.t_args("sharing.bindings.countAllBound", count)
  } else {
    ctx.t_args("sharing.bindings.count", count)
  };
  Column::new()
    .id("sharing-bindings-lead")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(&ctx.t("sharing.bindings.title"))
            .id("sharing-bindings-title")
            .variant(ProjectSettingsText::GroupTitle.role()),
        )
        .child(
          Text::new(&count)
            .id("sharing-bindings-count")
            .variant(ProjectSettingsText::Hint.role()),
        ),
    )
    .child(
      Text::new(&ctx.t("sharing.bindings.lead"))
        .id("sharing-bindings-text")
        .variant(ProjectSettingsText::Hint.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

/// The section, mounted on a shared project's Sharing page.
pub(super) fn section(ctx: &mut Ctx, project: &ShownProject) -> Element {
  ctx.mount_keyed::<OnThisComputer>(
    "sharing-on-this-computer",
    Props {
      project: project.clone(),
    },
  )
}

/// The picker's choice, bound once the input that chose it is the person's.
fn bind_picked(actions: &rows::Actions, choice: BoundReference) {
  if actions.person.allows(PICKED) {
    actions.bind.run(choice);
  }
}
