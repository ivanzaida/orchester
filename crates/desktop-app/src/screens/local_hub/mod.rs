//! The Agents hub of a local project (`Agents · v2` `acFkz`, L1–L14; `docs/design/team-configuration.md` rules 11 and
//! 12): the same master–detail page as a team's ([`super::team_hub`]) without the team: a filter by kind (All,
//! Profiles, MCP servers, Instructions, Skills, Tools) and *Archived*, a list of the project's resources grouped by
//! what the person has to do (*Needs you*, *Ready*, *Off*), and the panel of the chosen one. No From the team / On this
//! computer split, no badges of who may change what, no admin or member. A local MCP server has no versions and is
//! edited in place ([`mcp`]), and so are a profile ([`profile`]), an instruction ([`instruction`]) and a skill
//! ([`skill`]): each panel holds what is changed until *Save*, which publishes it as the next version at once, and
//! *Cancel* puts back what is stored ([`save_bar`]). Archiving a resource stops
//! the runs that use it and takes it out of the lists ([`archive`]); *Archived* lists what was archived and restores it
//! ([`archived`]). The filter and the chosen entry are the route's ([`HubView`]).
//!
//! The readiness is read off the UI thread through its query (`queries::hub::local_hub`), kept for the session: a visit
//! shows the last answer at once and reads it again behind it, as do a change of the project's resources or
//! connections and a run's start or end; never on a timer: a tool's or a server's secret is looked up in the keychain.
//! While it is read again the page shows the last answer as it is, and a read that failed shows its failure and no
//! counts. Before it was ever read the list and the panel stand in bones (`hub::agents_bones`).

pub(in crate::screens) mod archive;
mod archived;
mod editing;
mod head;
mod instruction;
mod leave;
mod list;
pub(in crate::screens) mod mcp;
mod model;
mod panel;
pub(in crate::screens) mod profile;
mod row;
mod save_bar;
mod skill;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
  },
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  resource::ResourceKind,
  time::Timestamp,
};
use orchester_workspace::{agents::local_hub::LocalHub, local_archive::Archivable, messages::Message};

pub(in crate::screens) use self::model::view_of;
use self::{
  archive::Asking,
  leave::Leave,
  mcp::UnsavedSignal,
  model::{Entry, Listing, chosen},
};
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  project::{ShownProject, TeamRead},
  queries::{self, Again, Session, hub as hub_query},
  routes::{HubFilter, HubView, ProjectPage, Route},
  screens::{
    hub::{
      agents, agents_bones,
      new_button::{made_under, new_button, new_words},
    },
    team_hub::{create, model::Kind, unread},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    failure::{Failure, LastFailure},
    icon::Icon,
    state::loading,
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
  /// Counts *Try again*, which reads the hub again.
  retry: Signal<u64>,
  again: Again,
  /// Why the hub couldn't be read, kept while it is read again.
  failure: LastFailure,
  /// The MCP server, instruction or skill whose panel has edits that are not saved, which its row says.
  unsaved: UnsavedSignal,
  /// Where the person pressed a row while edits are not saved: it waits for their answer ([`leave`]).
  leaving: Signal<Option<Route>>,
  /// The question that asks it.
  asking: Signal<bool>,
  /// The entry whose archive confirmation shows over the page, which outlives the entry's panel.
  archiving: Asking,
  /// Holds every other departure (Back, a tab, the sidebar) while a panel's edits are not saved.
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
      again: Again::default(),
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
      (None, None) => agents_bones::body(ctx, "local-hub-loading", ctx.t("local_hub.reading")),
    };
    let new = new_button(
      ctx,
      "local-hub-new",
      view.filter,
      leave.press(Route::Project(
        id,
        ProjectPage::hub(HubView::adding(made_under(view.filter))),
      )),
    );
    let leaving = listing.as_ref().and_then(|listing| {
      leave::question(
        ctx,
        (&self.leaving, &self.asking),
        (listing, &self.unsaved),
        (&navigation, &self.guard),
      )
    });
    let status = head::status(ctx);
    let tabs = tabs(id, listing.as_ref(), selected.as_ref().map(|entry| entry.what), &leave);
    let over: Vec<Element> = leaving.into_iter().chain(self.archiving.question(ctx)).collect();
    agents::frame(
      ctx,
      agents::Frame {
        prefix: "local-hub",
        filter: view.filter,
        status,
        new: Some(new),
        tabs,
        notices: Vec::new(),
        body,
        over,
      },
    )
  }
}

/// A tab per kind and *Archived* with its count (none while the hub is read), each keeping the chosen entry when it
/// holds it.
fn tabs(
  project: ProjectId,
  listing: Option<&Listing>,
  selected: Option<Archivable>,
  leave: &Leave,
) -> Vec<agents::Tab> {
  HubFilter::LOCAL
    .into_iter()
    .map(|choice| {
      let item = selected.filter(|item| listing.is_some_and(|listing| listing.holds(choice, *item)));
      let route = Route::Project(project, ProjectPage::hub(view_of(choice, item)));
      let leave = leave.clone();
      agents::Tab {
        filter: choice,
        count: listing.map(|listing| listing.count(choice)),
        open: Arc::new(move || leave.go(route.clone())),
      }
    })
    .collect()
}

impl LocalHubScreen {
  /// The hub as the project's database has it, from the session's cache: its last answer at once, read again behind it
  /// on each visit, when a connection, a tool, a profile, an instruction, a skill or a pipeline changed, and when a run
  /// changes state (`queries::changes`); and on *Try again*.
  fn read(&self, ctx: &mut Ctx, project: &ShownProject) -> FutureState<LocalHub, Message> {
    let session = Session::of(ctx);
    let query = ctx.query(hub_query::local_hub(session, project.open.repo_root.clone()));
    self.again.follow(self.retry.get(), &query);
    queries::reading(&query)
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
      return empty(ctx, view.filter, project.open.project.id(), shown.leave);
    }
    let list = list::list(ctx, listing, shown);
    let panel = match adding {
      Some(kind) => self.form(ctx, project, listing, kind),
      None => self.panel(ctx, (project, hub), selected),
    };
    agents::master_detail("local-hub", reading, (list, panel))
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
  fn panel(&self, ctx: &mut Ctx, (project, hub): (&ShownProject, &LocalHub), selected: Option<&Entry>) -> Element {
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
            unsaved: self.unsaved.clone(),
          },
        ),
        None => self.resource_panel(ctx, project, entry, id),
      },
    }
  }
}

impl LocalHubScreen {
  /// The panel of a resource other than a profile: an instruction and a skill are edited in it, a tool is shown.
  fn resource_panel(&self, ctx: &mut Ctx, project: &ShownProject, entry: &Entry, id: ResourceId) -> Element {
    match entry.kind {
      ResourceKind::Instruction => instruction::panel(
        ctx,
        instruction::Props {
          project: project.clone(),
          entry: entry.clone(),
          id,
          unsaved: self.unsaved.clone(),
        },
      ),
      ResourceKind::Skill => skill::panel(
        ctx,
        skill::Props {
          project: project.clone(),
          entry: entry.clone(),
          id,
          unsaved: self.unsaved.clone(),
        },
      ),
      _ => editing::panel(ctx, project, entry),
    }
  }
}

/// The open tab holds nothing: what the kind is and *New <kind>*; *Archived* has no action (archiving starts from a
/// resource's … menu).
fn empty(ctx: &Ctx, filter: HubFilter, project: ProjectId, leave: &Leave) -> Element {
  let (title, detail) = agents::empty_words(filter);
  let glyph = match filter {
    HubFilter::All => Icon::Bot,
    other => agents::glyph_of(other),
  };
  let action = (filter != HubFilter::Archived).then(|| {
    let form = Route::Project(project, ProjectPage::hub(HubView::adding(made_under(filter))));
    ActionButton::new("local-hub-empty-new", ButtonKind::Secondary, ctx.t(new_words(filter)))
      .icon(Icon::Plus)
      .press(leave.press(form))
      .into()
  });
  agents::empty(
    "local-hub",
    glyph,
    (&ctx.t(title), &ctx.t(detail)),
    action.into_iter().collect(),
  )
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
