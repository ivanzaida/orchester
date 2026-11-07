//! The Agents hub of a team project (`Agents · v2` `acFkz`, A1–A6; `docs/design/team-configuration.md`, notes rules 2
//! and 14): one master–detail page. A filter by kind (All, Profiles, MCP servers, Instructions, Skills, Tools) over a
//! list of the team's resources grouped by what this computer needs (*Needs you here*, *Ready here*, *Off here*) and
//! the panel of the chosen one, with the action that moves it on pinned at its bottom. The filter and the chosen
//! resource are the route's ([`HubView`]), so links, Back and Forward land on them; a route that names none shows the
//! first resource of the list. Instructions and skills need nothing of a computer and stand under *Ready here*.
//!
//! The readiness is read off the UI thread when the page opens and when the project's resources, fill-ins or team
//! standings change, never on a timer: a local tool's secret is read from the keychain.

mod head;
mod list;
mod model;
mod panel;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::Column,
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{agents::team_hub::TeamHub, background, messages::Message, tasks::outside::Outside};

use self::model::Listing;
use super::hub::team;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::HubView,
  services::Services,
  theme::HubSpacing,
  ui::{
    failure::{Failure, LastFailure},
    icon::Icon,
    state::{EmptyState, loading},
  },
};

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
  pub view: HubView,
}
impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, view: HubView) -> Element {
  ctx.mount_keyed::<TeamHubScreen>(
    &format!("team-hub-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      view,
    },
  )
}

struct TeamHubScreen {
  retry: Signal<u64>,
  /// Why the hub couldn't be read, kept while it is read again.
  failure: LastFailure,
}

impl Component for TeamHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retry: ctx.signal(0),
      failure: LastFailure::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view } = ctx.props::<Props>().clone();
    let Some(team) = team::of(&project) else {
      return Column::new().id("screen-team-hub-gone").into();
    };
    let outside = project.feed.outside.get();
    // Fill-ins (a program path, a secret value, On here) count as Connections; the rest are the kinds the hub lists.
    let dep = (
      self.retry.get(),
      outside.of(Outside::Connections),
      outside.of(Outside::Tools),
      outside.of(Outside::Profiles),
      outside.of(Outside::Instructions),
      outside.of(Outside::Skills),
      team.clone(),
    );
    let services = Services::of(ctx);
    let (workspace, vault, hold) = (services.workspace, services.vault, services.hold);
    let loaded = ctx
      .future((project.open.repo_root.clone(), dep), move |(root, _)| {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "team_hub");
        let (workspace, vault, hold) = (workspace.clone(), vault.clone(), hold.clone());
        async move {
          hold.released().await;
          background::run(move || workspace.team_hub(&root, vault.get()))
            .await?
            .map_err(|error| error.message())
        }
      })
      .state()
      .get();
    let failed = self.failure.follow(&loaded);
    let navigation = Navigation::of(ctx);
    let body = match (&loaded.data, failed) {
      (_, Some((error, retrying))) => self.failure_page(ctx, &project, error, retrying),
      (Some(hub), None) => hub_body(ctx, &project, (&team, hub), (&view, &navigation)),
      (None, None) => loading("team-hub-loading", &ctx.t("team_hub.reading")).into(),
    };
    let listing = loaded.data.as_ref().map(Listing::of).unwrap_or_default();
    frame(ctx, (&project, &team, &listing), (&view, &navigation), body)
  }
}

impl TeamHubScreen {
  fn failure_page(&self, ctx: &mut Ctx, project: &ShownProject, error: Message, retrying: bool) -> Element {
    let retry = self.retry.clone();
    Failure::new("team-hub-load-error", ctx.t("team_hub.loadFailed"), error)
      .retry(retrying, move || retry.update(|n| *n += 1))
      .log(project.open.repo_root.clone())
      .still_works(ctx.t("hub.failure.stillWorks"))
      .page(ctx)
  }
}

/// The page: the head and the filter over `body`.
fn frame(
  ctx: &Ctx,
  (project, team, listing): (
    &ShownProject,
    &orchester_workspace::sharing::team::TeamConfiguration,
    &Listing,
  ),
  (view, navigation): (&HubView, &Navigation),
  body: Element,
) -> Element {
  Column::new()
    .id("screen-team-hub")
    .describe("filter", view.filter.name())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(HubSpacing::HeadTop.role())
        .bottom(HubSpacing::HeadTop.role())
        .left(HubSpacing::Side.role())
        .right(HubSpacing::Side.role()),
    )
    .spacing(crate::theme::ExtraSpacing::Inset.role())
    .child(head::head(ctx, team))
    .child(head::filter(
      ctx,
      listing,
      (project.open.project.id(), view.filter, navigation),
    ))
    .child(
      Column::new()
        .id("team-hub-body")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(body),
    )
    .into()
}

/// The list and the panel, or what stands in for them when the filter holds nothing.
fn hub_body(
  ctx: &mut Ctx,
  project: &ShownProject,
  (team, hub): (&orchester_workspace::sharing::team::TeamConfiguration, &TeamHub),
  (view, navigation): (&HubView, &Navigation),
) -> Element {
  let listing = Listing::of(hub);
  let id = project.open.project.id();
  let selected = listing.selected(view.filter, view.item);
  let Some(selected) = selected else {
    return EmptyState::new(
      "team-hub-empty",
      Icon::Users,
      ctx.t("team_hub.empty.title"),
      ctx.t("team_hub.empty.detail"),
    )
    .into();
  };
  let list = list::list(
    ctx,
    &listing,
    &list::Shown {
      project: id,
      filter: view.filter,
      selected: Some(selected),
      navigation,
    },
  );
  let panel = panel::panel(
    ctx,
    &listing,
    selected,
    &panel::Shown {
      project: id,
      may_change: team.may_change,
      navigation,
    },
  );
  lurq::components::Row::new()
    .id("team-hub-master-detail")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Column::new()
        .width(Dimension::Px(list::LIST_WIDTH))
        .height(Dimension::Pct(100.0))
        .flex_shrink(0.0)
        .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
        .child(list),
    )
    .child(panel)
    .into()
}
