//! The Agents hub of a local project (`Agents · v2` `acFkz`, L1–L14; `docs/design/team-configuration.md` rules 11 and
//! 12): the same master–detail page as a team's ([`super::team_hub`]) without the team: a filter by kind (All,
//! Profiles, MCP servers, Instructions, Skills, Tools) and *Archived*, a list of the project's resources grouped by
//! what the person has to do (*Needs you*, *Ready*, *Off*), and the panel of the chosen one. No From the team / On this
//! computer split, no badges of who may change what, no admin or member. A local MCP server has no versions and is
//! edited in place ([`mcp`]); a profile, instruction, skill or tool opens the editor it has. Archiving a resource stops
//! the runs that use it and takes it out of the lists ([`archive`]); *Archived* lists what was archived and restores it
//! ([`archived`]). The filter and the chosen entry are the route's ([`HubView`]).
//!
//! The readiness is read off the UI thread when the page opens and when the project's resources or connections change,
//! never on a timer: a tool's or a server's secret is looked up in the keychain. While it is read again the page shows
//! the last answer dimmed with *Reading…*, and a read that failed shows its failure and no counts.

pub(in crate::screens) mod archive;
mod archived;
mod head;
mod list;
mod mcp;
mod model;
mod panel;
mod row;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
    theme::{BorderSize, PaletteColor, RadiusSize},
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::time::Timestamp;
use orchester_workspace::{
  agents::local_hub::LocalHub, background, local_archive::Archivable, messages::Message, tasks::outside::Outside,
};

use self::{
  head::FilterShown,
  mcp::UnsavedSignal,
  model::{Entry, Listing, chosen, view_of},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{HubFilter, HubView, ProjectPage, Route},
  screens::team_hub::list::LIST_WIDTH,
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
}
impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, view: HubView) -> Element {
  ctx.mount_keyed::<LocalHubScreen>(
    &format!("local-hub-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      view,
    },
  )
}

struct LocalHubScreen {
  retry: Signal<u64>,
  /// Why the hub couldn't be read, kept while it is read again.
  failure: LastFailure,
  /// The MCP server whose panel has edits that are not saved, which its row says.
  unsaved: UnsavedSignal,
}

impl Component for LocalHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retry: ctx.signal(0),
      failure: LastFailure::default(),
      unsaved: ctx.signal(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view } = ctx.props::<Props>().clone();
    let loaded = self.read(ctx, &project);
    let failed = self.failure.follow(&loaded);
    let reading = loaded.is_pending();
    let hub = loaded.data.as_ref().filter(|_| failed.is_none());
    let listing = hub.map(Listing::of);
    let navigation = Navigation::of(ctx);
    let selected = listing.as_ref().and_then(|listing| listing.selected(&view)).cloned();
    let id = project.open.project.id();
    let now = Timestamp::now();
    let body = match (failed, hub.zip(listing.as_ref())) {
      (Some((error, retrying)), _) => self.failure_page(ctx, &project, error, retrying),
      (None, Some((hub, listing))) => {
        if !reading {
          keep_route_on_shown(&project, &view, selected.as_ref(), &navigation);
        }
        let shown = list::Shown {
          project: id,
          filter: view.filter,
          selected: selected.as_ref().map(|entry| entry.what),
          navigation: &navigation,
          now,
          unsaved: self.unsaved.get(),
        };
        self.hub_body(
          ctx,
          (&project, hub, listing),
          (&view, &shown),
          (&navigation, reading),
          selected.as_ref(),
        )
      }
      (None, None) => loading("local-hub-loading", &ctx.t("local_hub.reading")).into(),
    };
    let shown = FilterShown {
      project: id,
      filter: view.filter,
      selected: selected.as_ref().map(|entry| entry.what),
      reading,
    };
    Column::new()
      .id("screen-local-hub")
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
      .child(head::head(ctx, id, view.filter, &navigation))
      .child(head::filter(ctx, listing.as_ref(), &shown, &navigation))
      .child(
        Column::new()
          .id("local-hub-body")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(body),
      )
  }
}

impl LocalHubScreen {
  /// The hub as the project's database has it: read again when a connection, a tool, a profile, an instruction, a skill
  /// or a pipeline changed.
  fn read(&self, ctx: &mut Ctx, project: &ShownProject) -> FutureState<LocalHub, Message> {
    let outside = project.feed.outside.get();
    let dep = (
      self.retry.get(),
      outside.of(Outside::Connections),
      outside.of(Outside::Tools),
      outside.of(Outside::Profiles),
      outside.of(Outside::Instructions),
      outside.of(Outside::Skills),
      outside.of(Outside::Pipelines),
    );
    let services = Services::of(ctx);
    let (workspace, vault, hold) = (services.workspace, services.vault, services.hold);
    ctx
      .future((project.open.repo_root.clone(), dep), move |(root, _)| {
        tracing::trace!(target: crate::app_log::READ_LOG, read = "local_hub");
        let (workspace, vault, hold) = (workspace.clone(), vault.clone(), hold.clone());
        async move {
          hold.released().await;
          background::run(move || workspace.local_hub(&root, vault.get()))
            .await?
            .map_err(|error| error.message())
        }
      })
      .state()
      .get()
  }

  fn failure_page(&self, ctx: &mut Ctx, project: &ShownProject, error: Message, retrying: bool) -> Element {
    let retry = self.retry.clone();
    Failure::new("local-hub-load-error", ctx.t("local_hub.loadFailed"), error)
      .retry(retrying, move || retry.update(|n| *n += 1))
      .log(project.open.repo_root.clone())
      .still_works(ctx.t("hub.failure.stillWorks"))
      .page(ctx)
  }

  /// The list and the panel, or what stands in for them when the filter holds nothing. `reading`: they show the last
  /// answer while it is read again.
  fn hub_body(
    &self,
    ctx: &mut Ctx,
    (project, hub, listing): (&ShownProject, &LocalHub, &Listing),
    (view, shown): (&HubView, &list::Shown<'_>),
    (navigation, reading): (&Navigation, bool),
    selected: Option<&Entry>,
  ) -> Element {
    if listing.count(view.filter) == 0 {
      let (title, detail) = if view.filter == HubFilter::Archived {
        ("local_hub.archived.emptyTitle", "local_hub.archived.emptyDetail")
      } else {
        ("local_hub.empty.title", "local_hub.empty.detail")
      };
      return EmptyState::new("local-hub-empty", Icon::Bot, ctx.t(title), ctx.t(detail)).into();
    }
    let list = list::list(ctx, listing, shown);
    let panel = self.panel(ctx, (project, hub), selected, navigation);
    Row::new()
      .id("local-hub-master-detail")
      .describe("state", if reading { "reading" } else { "read" })
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .opacity(if reading { HUB_READING_OPACITY } else { 1.0 })
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .child(
        Column::new()
          .width(Dimension::Px(LIST_WIDTH))
          .height(Dimension::Pct(100.0))
          .flex_shrink(0.0)
          .border_right(Border::inside(BorderSize::Sm, PaletteColor::Border))
          .child(list),
      )
      .child(panel)
      .into()
  }

  /// The panel of `selected`: an archived entry's, an MCP server's, or the frame the other kinds share.
  fn panel(
    &self,
    ctx: &mut Ctx,
    (project, hub): (&ShownProject, &LocalHub),
    selected: Option<&Entry>,
    navigation: &Navigation,
  ) -> Element {
    let Some(entry) = selected else {
      return archived::nothing_picked(ctx);
    };
    let id = project.open.project.id();
    if entry.archived.is_some() {
      let stopped = hub
        .archived()
        .iter()
        .find(|found| found.what == entry.what)
        .map(|found| found.stopped.clone())
        .unwrap_or_default();
      return archived::panel(
        ctx,
        archived::Props {
          root: project.open.repo_root.clone(),
          project: id,
          entry: entry.clone(),
          stopped,
          now: Timestamp::now(),
        },
      );
    }
    match entry.what {
      Archivable::McpServer(server) => mcp::panel(
        ctx,
        mcp::Props {
          project: project.clone(),
          entry: entry.clone(),
          server,
          unsaved: self.unsaved.clone(),
        },
      ),
      Archivable::Resource(_) => panel::panel(
        ctx,
        entry,
        &panel::Shown {
          project: id,
          root: &project.open.repo_root,
          navigation,
        },
      ),
    }
  }
}

/// A route that names an entry the filter does not hold (one archived, or of another kind) shows the one the page
/// shows; the route is corrected to it, in place, so a link, Back and a reload land on what the page says.
fn keep_route_on_shown(project: &ShownProject, view: &HubView, shown: Option<&Entry>, navigation: &Navigation) {
  let named = chosen(view);
  if named.is_some() && named != shown.map(|entry| entry.what) {
    let corrected = Route::Project(
      project.open.project.id(),
      ProjectPage::hub(view_of(view.filter, shown.map(|entry| entry.what))),
    );
    navigation.replace(corrected);
  }
}
