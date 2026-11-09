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
mod editing;
mod head;
mod leave;
mod list;
pub(in crate::screens) mod mcp;
mod model;
mod new_menu;
mod panel;
pub(in crate::screens) mod profile;
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

pub(in crate::screens) use self::model::view_of;
use self::{
  archive::Asking,
  head::FilterShown,
  leave::Leave,
  mcp::UnsavedSignal,
  model::{Entry, Listing, chosen},
};
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  project::{ShownProject, TeamRead},
  routes::{HubFilter, HubView, ProjectPage, Route},
  screens::team_hub::{create, list::LIST_WIDTH, model::Kind, unread},
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
  /// Where the person pressed a row while edits are not saved: it waits for their answer ([`leave`]).
  leaving: Signal<Option<Route>>,
  /// The question that asks it.
  asking: Signal<bool>,
  /// The entry whose archive confirmation shows over the page, which outlives the entry's panel.
  archiving: Asking,
  /// Holds every other departure (Back, a tab, the sidebar) while an MCP server's edits are not saved.
  guard: LeaveGuard,
}

impl Component for LocalHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let unsaved: UnsavedSignal = ctx.signal(None);
    let edits = unsaved.clone();
    let archiving = Asking::new(ctx);
    ctx.provide(archiving.clone());
    let this = Self {
      retry: ctx.signal(0),
      failure: LastFailure::default(),
      unsaved,
      leaving: ctx.signal(None),
      asking: ctx.signal(false),
      archiving,
      guard: Navigation::of(ctx).guard(move || edits.get_untracked().is_some()),
    };
    // The question follows what waits for an answer, and what waits follows the question closing; each writes only
    // when it differs, or they would write each other for ever.
    let asking = this.asking.clone();
    ctx.watch(&this.leaving, move |route| {
      if asking.get_untracked() != route.is_some() {
        asking.set(route.is_some());
      }
    });
    let asking = this.asking.clone();
    ctx.watch(&this.guard.attempt(), move |attempt| {
      if asking.get_untracked() != attempt.is_some() {
        asking.set(attempt.is_some());
      }
    });
    let (leaving, guard) = (this.leaving.clone(), this.guard.clone());
    ctx.watch(&this.asking, move |open| {
      if *open {
        return;
      }
      if leaving.get_untracked().is_some() {
        leaving.set(None);
      }
      if guard.attempt().get_untracked().is_some() {
        guard.keep_editing();
      }
    });
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view } = ctx.props::<Props>().clone();
    let loaded = self.read(ctx, &project);
    let failed = self.failure.follow(&loaded);
    let reading = loaded.is_pending();
    let hub = loaded.data.as_ref().filter(|_| failed.is_none());
    let listing = hub.map(Listing::of);
    let navigation = Navigation::of(ctx);
    let leave = Leave::new(&navigation, &self.leaving, self.unsaved.get().is_some());
    let selected = listing.as_ref().and_then(|listing| listing.selected(&view)).cloned();
    let id = project.open.project.id();
    let now = Timestamp::now();
    let body = match (failed, hub.zip(listing.as_ref())) {
      (Some((error, retrying)), _) => self.failure_page(ctx, &project, error, retrying),
      (None, Some((hub, listing))) => {
        if !reading && !view.adding {
          keep_route_on_shown(&project, &view, selected.as_ref(), &navigation);
        }
        let shown = list::Shown {
          project: id,
          filter: view.filter,
          selected: selected.as_ref().map(|entry| entry.what),
          leave: &leave,
          now,
          unsaved: self.unsaved.get(),
          adding: Kind::of_filter(view.filter).filter(|_| view.adding),
        };
        self.hub_body(
          ctx,
          (&project, hub, listing),
          (&view, &shown),
          reading,
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
    let leaving = listing.as_ref().and_then(|listing| {
      leave::question(
        ctx,
        (&self.leaving, &self.asking),
        (listing, &self.unsaved),
        (&navigation, &self.guard),
      )
    });
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
      .child(head::head(ctx, id, view.filter, &leave))
      .child(head::filter(ctx, listing.as_ref(), &shown, &leave))
      .child(
        Column::new()
          .id("local-hub-body")
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(body),
      )
      .with_children(leaving)
      .with_children(self.archiving.question(ctx))
  }
}

impl LocalHubScreen {
  /// The hub as the project's database has it: read again when a connection, a tool, a profile, an instruction, a skill
  /// or a pipeline changed, and when a run changes state.
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
      // What the runs decide: which profiles and servers they use, how many runs stop, and so readiness. A run starts
      // or ends, not a line of its transcript.
      project.feed.states.get(),
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
    reading: bool,
    selected: Option<&Entry>,
  ) -> Element {
    let adding = shown.adding;
    if listing.count(view.filter) == 0 && adding.is_none() {
      let (title, detail) = if view.filter == HubFilter::Archived {
        ("local_hub.archived.emptyTitle", "local_hub.archived.emptyDetail")
      } else {
        ("local_hub.empty.title", "local_hub.empty.detail")
      };
      return EmptyState::new("local-hub-empty", Icon::Bot, ctx.t(title), ctx.t(detail)).into();
    }
    let list = list::list(ctx, listing, shown);
    let panel = match adding {
      Some(kind) => self.form(ctx, project, listing, kind),
      None => self.panel(ctx, (project, hub), (selected, reading)),
    };
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

  /// The form that makes a new one of `kind` (`Agents · New agent profile` … `New tool`, L3–L7).
  fn form(&self, ctx: &mut Ctx, project: &ShownProject, listing: &Listing, kind: Kind) -> Element {
    create::panel(
      ctx,
      create::Props {
        repo_root: project.open.repo_root.clone(),
        project: project.open.project.id(),
        kind,
        mode: create::Mode::Local,
        taken: listing.names_of(kind.core()),
        team: None,
      },
    )
  }

  /// The panel of `selected`: an archived entry's, an MCP server's, or the frame the other kinds share.
  fn panel(
    &self,
    ctx: &mut Ctx,
    (project, hub): (&ShownProject, &LocalHub),
    (selected, reading): (Option<&Entry>, bool),
  ) -> Element {
    let Some(entry) = selected else {
      return archived::nothing_picked(ctx);
    };
    let id = project.open.project.id();
    if entry.archived.is_some() {
      let found = hub.archived().iter().find(|found| found.what == entry.what);
      let stopped = found.map(|found| found.stopped.clone()).unwrap_or_default();
      return archived::panel(
        ctx,
        archived::Props {
          root: project.open.repo_root.clone(),
          project: id,
          entry: entry.clone(),
          stopped,
          details: found.and_then(|found| found.details.clone()),
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
      Archivable::Resource(id) => match &entry.profile {
        Some(profile) => profile::panel(
          ctx,
          profile::Props {
            project: project.clone(),
            entry: entry.clone(),
            id,
            profile: profile.clone(),
            hub: hub.clone(),
            reading,
          },
        ),
        None => editing::panel(ctx, project, (entry, id)),
      },
    }
  }
}

/// The page that makes a new profile, MCP server, instruction, skill or tool, which in a local project is the form of
/// the hub (`Agents · v2` L3–L7): the route is replaced by it, in place, so Back and a reload land on the hub. A team's
/// project keeps `old`, the page it has; while it is not known which the project is, the page waits.
pub(in crate::screens) fn form_or(
  ctx: &mut Ctx,
  project: &ShownProject,
  filter: HubFilter,
  old: impl FnOnce(&mut Ctx) -> Element,
) -> Element {
  match project.feed.team.get() {
    TeamRead::Read(None) => {
      let form = Route::Project(project.open.project.id(), ProjectPage::hub(HubView::adding(filter)));
      Navigation::of(ctx).replace(form);
      loading("local-hub-form-opening", &ctx.t("local_hub.reading")).into()
    }
    TeamRead::Unread => loading("local-hub-form-opening", &ctx.t("local_hub.reading")).into(),
    TeamRead::Read(Some(_)) => old(ctx),
    TeamRead::Failed(error) => unread::failed(ctx, project, error),
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
