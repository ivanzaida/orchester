//! *Missing on this computer* (`04` `w0Evp`, decision 7 of `docs/design/shared-configuration.md`): a shared
//! resource's page says which reference it names that nothing here is bound to, and that what uses it can't run here;
//! a pipeline's page says the same of what its steps' profiles and tools name. *Choose on this computer…* opens
//! Sharing › On this computer, where the person binds it. Instructions name no reference, so they have no bar.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  node::Element,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  sharing::ConnectionRole,
};
use orchester_store::BoundReference;
use orchester_workspace::{
  background,
  sharing::sync::{Here, HereReference},
};

use super::kind_of;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  screens::{
    hub::draft_bar::{DraftBar, Standing},
    settings::orchester::{Said, follow_shared, sharing_said},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
  },
};

/// What the bar is on: a resource that names the references itself, or a pipeline whose steps' executors do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Whose {
  Resource,
  Pipeline,
}

/// The bar about `missing`, the references `name` needs that need the person: `{prefix}-bar`, with
/// `{prefix}-choose`.
pub(crate) fn bar(
  ctx: &mut Ctx,
  prefix: &str,
  missing: &[&HereReference],
  (project, name, whose): (ProjectId, &str, Whose),
) -> Option<Element> {
  let first = *missing.first()?;
  let reference = ("name", first.binding.name.clone());
  // The kind a connection's role names, *API connection “DeepSeek”*; a role this version does not know has none.
  let known = first
    .binding
    .role
    .as_deref()
    .and_then(ConnectionRole::parse)
    .map(ConnectionRole::as_str);
  let status = match (missing.len(), first.binding.reference, kind_of(ctx, known)) {
    (1, BoundReference::Connection { .. }, Some(kind)) => ctx.t_args(
      "sharing.config.missing.connectionOf",
      [reference, ("kind", kind.to_string())],
    ),
    (1, BoundReference::Connection { .. }, None) => ctx.t_args("sharing.config.missing.connection", [reference]),
    (1, BoundReference::McpServer { .. }, _) => ctx.t_args("sharing.config.missing.server", [reference]),
    (1, BoundReference::Secret { .. }, _) => ctx.t_args("sharing.config.missing.secret", [reference]),
    (count, ..) => ctx.t_args("sharing.config.missing.several", [("count", count.to_string())]),
  };
  let resource = ("resource", name.to_owned());
  let sub = match (whose, missing.len(), first.binding.reference, known) {
    (Whose::Pipeline, ..) => ctx.t_args("sharing.config.missing.bodyPipeline", [resource]),
    (Whose::Resource, 1, BoundReference::Connection { .. }, Some(role)) => ctx.t_args(
      "sharing.config.missing.bodyConnection",
      [
        resource,
        ("role", role.to_owned()),
        (
          "connection",
          ctx.t(&format!("sharing.bindings.aConnection.{role}")).to_string(),
        ),
      ],
    ),
    (Whose::Resource, ..) => ctx.t_args("sharing.config.missing.body", [resource]),
  };
  let open = Navigation::of(ctx).to(Route::Project(
    project,
    ProjectPage::Settings(ProjectSettingsTab::Sharing),
  ));
  Some(
    DraftBar::new(format!("{prefix}-bar"), Standing::Missing, status)
      .sub(sub)
      .action(
        ActionButton::new(
          format!("{prefix}-choose"),
          ButtonKind::Secondary,
          ctx.t("sharing.config.missing.choose"),
        )
        .icon(Icon::ArrowRight)
        .opens(open),
      )
      .into(),
  )
}

/// The page a bar is on.
pub(crate) struct Page<'a> {
  pub project: &'a ShownProject,
  /// The ids start with it: `{prefix}-bar`, `{prefix}-choose`.
  pub prefix: &'a str,
  /// The resource the page shows, then for a pipeline the profiles and tools its steps run; none for a new one.
  pub resources: Vec<ResourceId>,
  pub name: &'a str,
  pub whose: Whose,
}

/// What this computer has of the project's references, as read.
#[derive(Clone, Debug, PartialEq)]
struct Read(Here);

impl DevtoolsInspectable for Read {}

/// The bar while anything `page` needs is missing here, read anew when a shared project's cache changes; or why the
/// references could not be read. Asks for its future on every render, so the caller's futures keep their order.
pub(crate) fn missing_here(ctx: &mut Ctx, page: &Page<'_>) -> Option<Element> {
  let servers = Services::of(ctx).servers;
  let shared = follow_shared(ctx, &servers);
  let state = ctx
    .future((page.project.open.repo_root.clone(), shared), |(root, _)| async move {
      background::run(move || Here::read(&root))
        .await
        .map_err(|_| Said::new("cloud.errors.local"))?
        .map(Read)
        .map_err(|error| sharing_said(&error))
    })
    .state()
    .get();
  match (state.data, state.error) {
    _ if page.resources.is_empty() => None,
    (_, Some(reason)) => Some(field_error(&format!("{}-unread", page.prefix), &reason.text(ctx)).into()),
    (Some(Read(here)), None) => {
      let mut missing: Vec<&HereReference> = Vec::new();
      for resource in &page.resources {
        for reference in here.missing_for(*resource) {
          if !missing.iter().any(|kept| std::ptr::eq(*kept, reference)) {
            missing.push(reference);
          }
        }
      }
      bar(
        ctx,
        page.prefix,
        &missing,
        (page.project.open.project.id(), page.name, page.whose),
      )
    }
    (None, None) => None,
  }
}
