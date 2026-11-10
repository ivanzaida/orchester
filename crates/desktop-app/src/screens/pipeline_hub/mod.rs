//! The Pipelines hub of a local project (`Pipelines · v2` `pCzIS`, P1): the same master–detail page as the Agents hub
//! ([`super::local_hub`]): a filter (*All*, *Drafts*, *Archived*) with counts, a list of the project's pipelines
//! grouped by whether they can run as they are (*Needs you*, *Ready*), and the panel of the chosen one ([`panel`]). The
//! tab and the chosen pipeline are the route's ([`PipelinesView`]). A team project keeps the page it has
//! ([`super::pipelines`]).
//!
//! What the panel's body holds is [`steps`]: today the way into the editor the pipeline has; the flow, the settings and
//! the Save bar are built on it.
//!
//! The pipelines are read off the UI thread through their query (`queries::hub::pipeline_hub`), kept for the session: a
//! visit shows the last answer at once and reads it again behind it, as do a change of the project's pipelines,
//! profiles, tools or connections and a run's start or end; never on a timer. A read that failed shows its failure and
//! no counts.

mod list;
mod model;
mod panel;
mod row;
mod steps;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureState},
  },
  components::Row,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ProjectId, ResourceId},
  time::Timestamp,
};
use orchester_workspace::{messages::Message, pipelines::hub::PipelineHub};

use self::model::{Entry, Listing};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  queries::{self, Again, Session, hub as hub_query},
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::{hub::agents, local_hub::head},
  theme::HubSpacing,
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
  pub view: PipelinesView,
}
impl DevtoolsInspectable for Props {}

pub fn screen(ctx: &mut Ctx, project: &ShownProject, view: PipelinesView) -> Element {
  ctx.mount_keyed::<PipelineHubScreen>(
    &format!("pipeline-hub-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      view,
    },
  )
}

struct PipelineHubScreen {
  /// Counts *Try again*, which reads the pipelines again.
  retry: Signal<u64>,
  again: Again,
  /// Why the pipelines couldn't be read, kept while they are read again.
  failure: LastFailure,
}

impl Component for PipelineHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      retry: ctx.signal(0),
      again: Again::default(),
      failure: LastFailure::default(),
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
    let selected = listing.as_ref().and_then(|listing| listing.selected(view)).cloned();
    let id = project.open.project.id();
    let body = match (failed, listing.as_ref()) {
      (Some((error, retrying)), _) => self.failure_page(ctx, &project, error, retrying),
      (None, Some(listing)) => {
        if !reading {
          keep_route_on_shown(id, view, selected.as_ref(), &navigation);
        }
        hub_body(
          ctx,
          &project,
          (listing, view),
          (selected.as_ref(), &navigation, reading),
        )
      }
      (None, None) => loading("pipeline-hub-loading", &ctx.t("pipeline_hub.reading")).into(),
    };
    let new = new_button(ctx, ("pipeline-hub-new", ButtonKind::Primary), &navigation, id);
    agents::frame(
      ctx,
      agents::Frame {
        prefix: "pipeline-hub",
        title: "pipeline_hub.title",
        current: view.filter.name(),
        status: head::status(ctx),
        new: Some(new),
        tabs: tabs(
          id,
          listing.as_ref(),
          selected.as_ref().map(|entry| entry.id),
          &navigation,
        ),
        notices: Vec::new(),
        body,
        over: Vec::new(),
      },
    )
  }
}

impl PipelineHubScreen {
  /// The pipelines as the project's database has them, from the session's cache: the last answer at once, read again
  /// behind it on each visit, when a pipeline, a profile, a tool or a connection changed and when a run changes state
  /// (`queries::changes`); and on *Try again*.
  fn read(&self, ctx: &mut Ctx, project: &ShownProject) -> FutureState<PipelineHub, Message> {
    let session = Session::of(ctx);
    let query = ctx.query(hub_query::pipeline_hub(session, project.open.repo_root.clone()));
    self.again.follow(self.retry.get(), &query);
    queries::reading(&query)
  }

  fn failure_page(&self, ctx: &mut Ctx, project: &ShownProject, error: Message, retrying: bool) -> Element {
    let retry = self.retry.clone();
    Failure::new("pipeline-hub-load-error", ctx.t("pipeline_hub.loadFailed"), error)
      .retry(retrying, move || retry.update(|n| *n += 1))
      .log(project.open.repo_root.clone())
      .still_works(ctx.t("hub.failure.stillWorks"))
      .page(ctx)
  }
}

/// The list and the panel, or what stands in for them when the tab holds nothing. `reading`: they show the last answer
/// while it is read again.
fn hub_body(
  ctx: &mut Ctx,
  project: &ShownProject,
  (listing, view): (&Listing, PipelinesView),
  (selected, navigation, reading): (Option<&Entry>, &Navigation, bool),
) -> Element {
  let id = project.open.project.id();
  let Some(entry) = selected else {
    return empty(ctx, view.filter, navigation, id);
  };
  let now = Timestamp::now();
  let shown = list::Shown {
    project: id,
    filter: view.filter,
    selected: Some(entry.id),
    navigation,
    now,
  };
  let list = list::list(ctx, listing, &shown);
  let panel = panel::panel(ctx, project, entry, now);
  agents::master_detail("pipeline-hub", reading, (list, panel))
}

/// *New pipeline*, which opens the page that makes one: the head's is the primary button, the empty body's the
/// secondary.
fn new_button(ctx: &Ctx, (id, kind): (&str, ButtonKind), navigation: &Navigation, project: ProjectId) -> Element {
  ActionButton::new(id.to_owned(), kind, ctx.t("pipeline_hub.new"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::Project(project, ProjectPage::NewPipeline)))
    .into()
}

/// The open tab holds nothing: what a pipeline is, with *New pipeline* when the project has none.
fn empty(ctx: &Ctx, filter: PipelinesFilter, navigation: &Navigation, project: ProjectId) -> Element {
  let (glyph, title, detail) = match filter {
    PipelinesFilter::All => (
      Icon::Workflow,
      "pipeline_hub.empty.all.title",
      "pipeline_hub.empty.all.detail",
    ),
    PipelinesFilter::Drafts => (
      Icon::FilePen,
      "pipeline_hub.empty.drafts.title",
      "pipeline_hub.empty.drafts.detail",
    ),
    PipelinesFilter::Archived => (
      Icon::Archive,
      "pipeline_hub.empty.archived.title",
      "pipeline_hub.empty.archived.detail",
    ),
  };
  let action = (filter == PipelinesFilter::All).then(|| {
    [
      // The design puts a gap between the line and the button (`Action gap` `BAEia` in P6), on top of the 10
      // either side of it.
      Row::new()
        .id("pipeline-hub-empty-action-gap")
        .height(Dimension::Px(HubSpacing::ActionGap.value()))
        .into(),
      new_button(
        ctx,
        ("pipeline-hub-empty-new", ButtonKind::Secondary),
        navigation,
        project,
      ),
    ]
  });
  agents::empty(
    "pipeline-hub",
    glyph,
    (&ctx.t(title), &ctx.t(detail)),
    action.into_iter().flatten().collect(),
  )
}

/// A tab per filter with its count (none while the pipelines are read), each keeping the chosen pipeline when it holds
/// it.
fn tabs(
  project: ProjectId,
  listing: Option<&Listing>,
  selected: Option<ResourceId>,
  navigation: &Navigation,
) -> Vec<agents::Tab> {
  PipelinesFilter::ALL
    .into_iter()
    .map(|choice| {
      let item = selected.filter(|item| listing.is_some_and(|listing| listing.holds(choice, *item)));
      let route = Route::Project(project, ProjectPage::pipelines(PipelinesView::new(choice, item)));
      let navigation = navigation.clone();
      agents::Tab {
        name: choice.name(),
        label: label_key(choice),
        count: listing.map(|listing| listing.count(choice)),
        open: Arc::new(move || navigation.go(route.clone())),
      }
    })
    .collect()
}

const fn label_key(filter: PipelinesFilter) -> &'static str {
  match filter {
    PipelinesFilter::All => "pipeline_hub.filter.all",
    PipelinesFilter::Drafts => "pipeline_hub.filter.drafts",
    PipelinesFilter::Archived => "pipeline_hub.filter.archived",
  }
}

/// A route that names a pipeline the tab does not hold (one archived, or not a draft) shows the one the page shows; the
/// route is corrected to it, in place, so a link, Back and a reload land on what the page says.
fn keep_route_on_shown(project: ProjectId, view: PipelinesView, shown: Option<&Entry>, navigation: &Navigation) {
  if view.item.is_some() && view.item != shown.map(|entry| entry.id) {
    let corrected = Route::Project(
      project,
      ProjectPage::pipelines(PipelinesView::new(view.filter, shown.map(|entry| entry.id))),
    );
    navigation.replace(corrected);
  }
}
