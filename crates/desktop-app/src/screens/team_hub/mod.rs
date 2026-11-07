//! The Agents hub of a team project (`Agents · v2` `acFkz`, A1–A6; `docs/design/team-configuration.md`, notes rules 2
//! and 14): one master–detail page. A filter by kind (All, Profiles, MCP servers, Instructions, Skills, Tools) over a
//! list of the team's resources grouped by what this computer needs (*Needs you here*, *Ready here*, *Off here*) and
//! the panel of the chosen one, with the action that moves it on pinned at its bottom. The filter and the chosen
//! resource are the route's ([`HubView`]), so links, Back and Forward land on them; a route that names none shows the
//! first resource of the list. Instructions and skills need nothing of a computer and stand under *Ready here*. An
//! admin has New in the head ([`create`]).
//!
//! The readiness is read off the UI thread when the page opens and when the project's resources, fill-ins or team
//! standings change, never on a timer: a local tool's secret is read from the keychain. While it is read again the
//! page shows the last answer dimmed with *Reading…*, and a read that failed shows its failure and no counts, never the
//! last answer as current. Before the project is known to be shared the page is [`unread`]'s.

mod create;
mod head;
mod list;
mod model;
mod panel;
mod row;
pub(super) mod unread;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{background, messages::Message, sharing::team::TeamConfiguration, tasks::outside::Outside};

use self::{create::NewMenu, head::FilterShown, model::Listing};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{HubView, ProjectPage, Route},
  services::Services,
  theme::{HUB_READING_OPACITY, HubSpacing},
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
  /// The team's configuration, which the dispatch has read: the project is shared.
  pub team: TeamConfiguration,
}
impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, team: TeamConfiguration, view: HubView) -> Element {
  ctx.mount_keyed::<TeamHubScreen>(
    &format!("team-hub-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      view,
      team,
    },
  )
}

struct TeamHubScreen {
  retry: Signal<u64>,
  /// Why the hub couldn't be read, kept while it is read again.
  failure: LastFailure,
  /// The menu All's New opens in the head.
  new_menu: NewMenu,
}

impl Component for TeamHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retry: ctx.signal(0),
      failure: LastFailure::default(),
      new_menu: NewMenu {
        open: ctx.signal(false),
        anchor: ctx.element_ref(),
      },
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view, team } = ctx.props::<Props>().clone();
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
    // A read under way or failed keeps the last answer in `loaded`: it counts and lists only while the read is not
    // failed, and a read under way marks what shows as the last answer.
    let reading = loaded.is_pending();
    let listing = loaded.data.as_ref().filter(|_| failed.is_none()).map(Listing::of);
    let navigation = Navigation::of(ctx);
    let selected = listing
      .as_ref()
      .and_then(|listing| listing.selected(view.filter, view.item))
      .map(|entry| entry.id);
    let body = match (failed, &listing) {
      (Some((error, retrying)), _) => self.failure_page(ctx, &project, error, retrying),
      (None, Some(listing)) => {
        if !reading {
          keep_route_on_shown(&project, view, selected, &navigation);
        }
        hub_body(ctx, (&project, &team), listing, (&view, &navigation), reading)
      }
      (None, None) => loading("team-hub-loading", &ctx.t("team_hub.reading")).into(),
    };
    let id = project.open.project.id();
    let new = team
      .may_change
      .then(|| create::new_button(ctx, (id, view.filter), (&navigation, &self.new_menu)))
      .flatten();
    let shown = FilterShown {
      project: id,
      filter: view.filter,
      selected,
      reading,
    };
    frame(ctx, (&team, listing.as_ref(), &shown), new, &navigation, (body, view))
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

/// A route that names a resource the filter does not hold (one removed, or of another kind) shows the first one of the
/// list; the route is corrected to the one shown, in place, so a link, Back and a reload land on what the page says.
fn keep_route_on_shown(
  project: &ShownProject,
  view: HubView,
  shown: Option<orchester_core::id::ResourceId>,
  navigation: &Navigation,
) {
  if view.item.is_some() && view.item != shown {
    let corrected = Route::Project(
      project.open.project.id(),
      ProjectPage::hub(HubView::new(view.filter, shown)),
    );
    navigation.replace(corrected);
  }
}

/// The page: the head and the filter over `body`.
fn frame(
  ctx: &mut Ctx,
  (team, listing, shown): (&TeamConfiguration, Option<&Listing>, &FilterShown),
  new: Option<Element>,
  navigation: &Navigation,
  (body, view): (Element, HubView),
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
    .child(head::head(ctx, team, new))
    .child(head::filter(ctx, listing, shown, navigation))
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

/// The list and the panel, or what stands in for them when the filter holds nothing. `reading`: they show the last
/// answer while it is read again.
fn hub_body(
  ctx: &mut Ctx,
  (project, team): (&ShownProject, &TeamConfiguration),
  listing: &Listing,
  (view, navigation): (&HubView, &Navigation),
  reading: bool,
) -> Element {
  let id = project.open.project.id();
  let Some(selected) = listing.selected(view.filter, view.item) else {
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
    listing,
    &list::Shown {
      project: id,
      filter: view.filter,
      selected: Some(selected),
      navigation,
    },
  );
  let panel = panel::panel(
    ctx,
    listing,
    selected,
    &panel::Shown {
      project: id,
      may_change: team.may_change,
      navigation,
    },
  );
  Row::new()
    .id("team-hub-master-detail")
    .describe("state", if reading { "reading" } else { "read" })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .opacity(if reading { HUB_READING_OPACITY } else { 1.0 })
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
