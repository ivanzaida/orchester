//! The Agents hub of a team project (`Agents · v2` `acFkz`, A1–A6; `docs/design/team-configuration.md`, notes rules 2
//! and 14): one master–detail page. A filter by kind (All, Profiles, MCP servers, Instructions, Skills, Tools) over a
//! list of the team's resources grouped by what this computer needs (*Needs you here*, *Ready here*, *Off here*) and
//! the panel of the chosen one, with the action that moves it on pinned at its bottom. The filter and the chosen
//! resource are the route's ([`HubView`]), so links, Back and Forward land on them; a route that names none shows the
//! first resource of the list. Instructions and skills need nothing of a computer and stand under *Ready here*. An
//! admin has New in the head ([`create`]).
//!
//! The readiness is read off the UI thread through its query (`queries::hub::team_hub`), kept for the session: a visit
//! shows the last answer at once and reads it again behind it, as do a change of the project's resources or fill-ins
//! and of the team's configuration, which the team server's events change (`queries::follow::ProjectReads`); never on a
//! timer: a local tool's secret is read from the keychain. While it is read again the page shows the last answer as it
//! is, and a read that failed shows its failure and no counts, never the last answer as current. Before it was ever
//! read the list and the panel stand in bones ([`agents_bones`]). Before the project is known to be shared the page is
//! [`unread`]'s.
//!
//! The team's configuration is read from its team server, held in memory for the session
//! (`docs/design/team-configuration-live.md`). When the server cannot be reached and nothing was read, the body says so
//! with the reason and *Try again*, the head says the team cannot be reached and has no *New*, and the tabs have no
//! counts ([`unreachable`], A25 `Zhcif`). When it was read and the server is lost since, what was read stays shown,
//! the head says the team cannot be reached, and nothing is created or published until it is back: the page reads as
//! it does for whoever may not change the team's resources.

pub(in crate::screens) mod actions;
pub(in crate::screens) mod archive;
pub(in crate::screens) mod bar;
pub(in crate::screens) mod create;
mod fill;
mod head;
pub(in crate::screens) mod list;
mod mcp;
mod member;
pub(in crate::screens) mod model;
pub(in crate::screens) mod panel;
mod plain;
pub(in crate::screens) mod profile;
pub(in crate::screens) mod row;
pub(in crate::screens) mod rows;
mod secret;
mod tool;
mod unreachable;
pub(super) mod unread;
pub(in crate::screens) mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_core::id::{ProjectId, ResourceId};
use orchester_workspace::{
  agents::team_hub::{TeamHub, TeamHubRead},
  messages::Message,
  sharing::team::TeamConfiguration,
  team_config::Unreached,
};

use self::model::{Kind, Listing};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  queries::{self, Again, Session, hub as hub_query},
  routes::{HubFilter, HubView, ProjectPage, Route},
  screens::hub::{
    agents::{self, TabCount},
    agents_bones,
    new_button::{made_under, new_button, new_words},
  },
  services::Services,
  team::reached::Reached,
  ui::{
    button::{ActionButton, ButtonKind},
    failure::{Failure, LastFailure},
    icon::Icon,
    pressable::Press,
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
  /// Counts *Try again* and *Check again*, each of which reads the hub again.
  retry: Signal<u64>,
  again: Again,
  /// Why the hub couldn't be read, kept while it is read again.
  failure: LastFailure,
}

impl Component for TeamHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retry: ctx.signal(0),
      again: Again::default(),
      failure: LastFailure::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view, team } = ctx.props::<Props>().clone();
    let session = Session::of(ctx);
    let query = ctx.query(hub_query::team_hub(session, project.open.repo_root.clone()));
    self.again.follow(self.retry.get(), &query);
    let loaded = queries::reading(&query);
    let failed = self.failure.follow(&loaded);
    // A read under way or failed keeps the last answer in `loaded`: it counts and lists only while the read is not
    // failed, and a read under way marks what shows as the last answer.
    let reading = loaded.is_pending();
    let (hub, unreached) = read_of(loaded.data.as_ref().filter(|_| failed.is_none()));
    let nothing_read = hub.is_none() && unreached.is_some();
    // Nothing is created or saved while the team server cannot be reached; a form an admin opened stays open.
    let admin = team.may_change;
    let team = TeamConfiguration {
      may_change: team.may_change && unreached.is_none(),
      ..team
    };
    let listing = hub.map(Listing::of);
    let navigation = Navigation::of(ctx);
    // A form has no resource chosen: the filter's links go to the kind's list.
    let selected = listing
      .as_ref()
      .filter(|_| !view.adding)
      .and_then(|listing| listing.selected(view.filter, view.item))
      .map(|entry| entry.id);
    let body = match (failed, hub.zip(listing.as_ref())) {
      (Some((error, retrying)), _) => self.failure_page(ctx, &project, error, retrying),
      (None, Some((hub, listing))) => {
        if !reading && !read_behind(ctx, &project, view, listing) {
          keep_route_on_shown(&project, view, selected, &navigation);
        }
        hub_body(
          ctx,
          (&project, &team, (admin, unreached.as_ref())),
          (hub, listing),
          (&view, &navigation),
          (reading, &self.retry),
        )
      }
      (None, None) => match unreached.as_ref() {
        Some(unreached) => self.unreachable_page(ctx, (&project, &team), unreached),
        None => agents_bones::body(ctx, "team-hub-loading", ctx.t("team_hub.reading")),
      },
    };
    let id = project.open.project.id();
    // Only an admin makes a resource: *New <kind>* opens the form of the kind the tab holds.
    let new = team.may_change.then(|| {
      let form = Route::Project(id, ProjectPage::hub(HubView::adding(made_under(view.filter))));
      new_button(ctx, "team-hub-new", view.filter, Press::Open(navigation.to(form)))
    });
    let reach = if unreached.is_some() {
      head::Reach::Unreachable
    } else if Services::of(ctx).team.reached().of(&project.open.repo_root) == Some(Reached::Unanswered) {
      head::Reach::Unanswered
    } else {
      head::Reach::Answered
    };
    let status = head::status(ctx, &team, reach);
    agents::frame(
      ctx,
      agents::Frame {
        prefix: "team-hub",
        filter: view.filter,
        status,
        new,
        tabs: tabs(id, (listing.as_ref(), nothing_read), selected, &navigation),
        notices: Vec::new(),
        body,
        over: Vec::new(),
      },
    )
  }
}

impl TeamHubScreen {
  /// The body while the team server cannot be reached and nothing was read: *Try again* asks it again now.
  fn unreachable_page(
    &self,
    ctx: &mut Ctx,
    (project, team): (&ShownProject, &TeamConfiguration),
    unreached: &Unreached,
  ) -> Element {
    let (services, root) = (Services::of(ctx), project.open.repo_root.clone());
    let retry = self.retry.clone();
    unreachable::body(ctx, team, unreached, move || {
      again(&services, &root);
      retry.update(|n| *n += 1);
    })
  }

  fn failure_page(&self, ctx: &mut Ctx, project: &ShownProject, error: Message, retrying: bool) -> Element {
    let retry = self.retry.clone();
    Failure::new("team-hub-load-error", ctx.t("team_hub.loadFailed"), error)
      .retry(retrying, move || retry.update(|n| *n += 1))
      .log(project.open.repo_root.clone())
      .still_works(ctx.t("hub.failure.stillWorks"))
      .page(ctx)
  }
}

/// Whether the person is an admin of the team, reachable or not, and why its server cannot be reached now.
type Admin<'a> = (bool, Option<&'a Unreached>);

/// The hub `read` holds, and why the team server cannot be reached; neither while the hub is being read.
fn read_of(read: Option<&TeamHubRead>) -> (Option<&TeamHub>, Option<Unreached>) {
  match read {
    Some(TeamHubRead::Read(hub)) => (Some(&**hub), hub.unreached().cloned()),
    Some(TeamHubRead::Unreachable(unreached)) => (None, Some(unreached.clone())),
    Some(TeamHubRead::Reading) | None => (None, None),
  }
}

/// Asks the team server of the project at `root` again now, off the UI thread: it opens the project's database.
fn again(services: &Services, root: &std::path::Path) {
  let (servers, root) = (services.servers.clone(), root.to_owned());
  services.runtime.spawn(async move {
    let asked = orchester_workspace::background::run(move || servers.retry_project(&root)).await;
    match asked {
      Ok(Ok(())) => {}
      Ok(Err(error)) => tracing::warn!(%error, "the team server of a project could not be asked again"),
      // The blocking pool is shutting down with the window.
      Err(_) => {}
    }
  });
}

/// Whether the route names a resource the team's configuration holds that the hub's answer does not list yet: one just
/// created, which the hub reads again behind. The route stays on it.
fn read_behind(ctx: &mut Ctx, project: &ShownProject, view: HubView, listing: &Listing) -> bool {
  view
    .item
    .filter(|item| listing.find(*item).is_none())
    .is_some_and(|item| {
      Services::of(ctx)
        .workspace
        .team_read(&project.open.repo_root)
        .config()
        .is_some_and(|config| config.live(item).is_some())
    })
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

/// A tab per kind with its count (its bone while the hub is read, none when nothing could be read), each keeping the
/// chosen resource when it holds it.
fn tabs(
  project: ProjectId,
  (listing, nothing_read): (Option<&Listing>, bool),
  selected: Option<ResourceId>,
  navigation: &Navigation,
) -> Vec<agents::Tab> {
  HubFilter::ALL
    .into_iter()
    .map(|choice| {
      let item = selected.filter(|item| listing.is_some_and(|listing| listing.holds(choice, *item)));
      let route = Route::Project(project, ProjectPage::hub(HubView::new(choice, item)));
      let navigation = navigation.clone();
      agents::Tab {
        filter: choice,
        count: if nothing_read {
          TabCount::Unknown
        } else {
          TabCount::of(listing.map(|listing| listing.count(choice)))
        },
        open: Arc::new(move || navigation.go(route.clone())),
      }
    })
    .collect()
}

/// The list and the panel, or what stands in for them when the filter holds nothing. `reading`: they show the last
/// answer while it is read again.
fn hub_body(
  ctx: &mut Ctx,
  (project, team, reach): (&ShownProject, &TeamConfiguration, Admin<'_>),
  (hub, listing): (&TeamHub, &Listing),
  (view, navigation): (&HubView, &Navigation),
  (reading, retry): (bool, &Signal<u64>),
) -> Element {
  let id = project.open.project.id();
  // Only an admin creates: a member who follows a link to a form sees the kind's list.
  let adding = Kind::of_filter(view.filter).filter(|_| view.adding && team.may_change);
  let selected = listing.selected(view.filter, view.item).filter(|_| adding.is_none());
  if selected.is_none() && adding.is_none() {
    return empty(ctx, view.filter, team, navigation, id);
  }
  let list = list::list(
    ctx,
    listing,
    &list::Shown {
      project: id,
      filter: view.filter,
      selected,
      adding,
      navigation,
    },
  );
  let panel = match (adding, selected) {
    (Some(kind), _) => create::panel(
      ctx,
      create::Props {
        repo_root: project.open.repo_root.clone(),
        project: id,
        kind,
        mode: create::Mode::Team,
        taken: listing.shown(kind.filter()).map(|entry| entry.name.clone()).collect(),
        team: Some(team.clone()),
      },
    ),
    (None, Some(selected)) => {
      let shown = panel::Shown { team };
      detail(
        ctx,
        (project, team, reach),
        (hub, listing, selected),
        (&shown, reading, retry),
      )
    }
    (None, None) => return Element::from(Row::new()),
  };
  agents::master_detail("team-hub", reading, (list, panel))
}

/// The open tab holds nothing: for an admin, what the kind is and *New <kind>*; for a member, that an admin adds it and
/// who the admins are, with no action.
fn empty(
  ctx: &Ctx,
  filter: HubFilter,
  team: &TeamConfiguration,
  navigation: &Navigation,
  project: ProjectId,
) -> Element {
  let glyph = agents::glyph_of(filter);
  if !team.may_change {
    let (title, detail) = member::empty_words(filter);
    let admins = member::admins_line(ctx, team);
    return agents::empty("team-hub", glyph, (&ctx.t(title), &ctx.t(detail)), vec![admins]);
  }
  let (title, detail) = agents::empty_words(filter);
  let form = Route::Project(project, ProjectPage::hub(HubView::adding(made_under(filter))));
  let action = ActionButton::new("team-hub-empty-new", ButtonKind::Secondary, ctx.t(new_words(filter)))
    .icon(Icon::Plus)
    .opens(navigation.to(form));
  agents::empty("team-hub", glyph, (&ctx.t(title), &ctx.t(detail)), vec![action.into()])
}

/// The panel of `selected`: the one of its kind where the team holds what it shows, else the frame every kind has.
fn detail(
  ctx: &mut Ctx,
  (project, team, (admin, unreached)): (&ShownProject, &TeamConfiguration, Admin<'_>),
  (hub, listing, selected): (&TeamHub, &Listing, &model::Entry),
  (shown, reading, retry): (&panel::Shown<'_>, bool, &Signal<u64>),
) -> Element {
  let id = project.open.project.id();
  let repo_root = project.open.repo_root.clone();
  let plain = |sheet| plain::Props {
    repo_root: repo_root.clone(),
    team: team.clone(),
    listing: listing.clone(),
    entry: selected.clone(),
    sheet,
  };
  match selected.kind {
    model::Kind::Profile => hub.profile(selected.id).map(|sheet| {
      profile::panel(
        ctx,
        profile::Props {
          repo_root,
          team: team.clone(),
          reading,
          listing: listing.clone(),
          entry: selected.clone(),
          sheet: sheet.clone(),
        },
      )
    }),
    model::Kind::McpServer => hub.mcp_server(selected.id).map(|sheet| {
      mcp::panel(
        ctx,
        mcp::Props {
          repo_root,
          project: id,
          team: team.clone(),
          admin,
          unreached: unreached.map(|unreached| unreached.reason.clone()),
          reading,
          reread: retry.clone(),
          listing: listing.clone(),
          entry: selected.clone(),
          sheet: sheet.clone(),
        },
      )
    }),
    model::Kind::Tool => hub.tool(selected.id).map(|sheet| {
      tool::panel(
        ctx,
        tool::Props {
          repo_root,
          team: team.clone(),
          listing: listing.clone(),
          entry: selected.clone(),
          sheet: sheet.clone(),
        },
      )
    }),
    model::Kind::Instruction => hub
      .instruction(selected.id)
      .map(|sheet| plain::panel(ctx, plain(plain::Sheet::Instruction(sheet.clone())))),
    model::Kind::Skill => hub
      .skill(selected.id)
      .map(|sheet| plain::panel(ctx, plain(plain::Sheet::Skill(sheet.clone())))),
  }
  .unwrap_or_else(|| panel::panel(ctx, listing, selected, shown))
}
