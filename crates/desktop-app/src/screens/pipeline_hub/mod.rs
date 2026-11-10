//! The Pipelines hub of a local project (`Pipelines · v2` `pCzIS`, P1): the same master–detail page as the Agents hub
//! ([`super::local_hub`]): a filter (*All*, *Drafts*, *Archived*) with counts, a list of the project's pipelines
//! grouped by whether they can run as they are (*Needs you*, *Ready*), and the panel of the chosen one ([`panel`]). The
//! tab and the chosen pipeline are the route's ([`PipelinesView`]). A team project keeps the page it has
//! ([`super::pipelines`]).
//!
//! The panel's head holds the ⋯ menu ([`panel_head`], [`menu`]): *Rename* in place ([`rename`]), *Duplicate*, *Make
//! default for New task* and *Archive*, whose confirmation the hub hosts so that it outlives the panel; an archived
//! pipeline's panel has *Restore* ([`archived`]). The default pipeline is tagged in its row and in the panel's chips.
//! *Rename* and *Make default* keep the edit the panel holds; *Duplicate* and *Archive* leave the panel, so they ask
//! first while it has changes ([`leave`]).
//!
//! What the panel's body holds is [`steps`]: today the way into the editor the pipeline has; the flow, the settings and
//! the Save bar are built on it.
//!
//! The pipelines are read off the UI thread through their query (`queries::hub::pipeline_hub`), kept for the session: a
//! visit shows the last answer at once and reads it again behind it, as do a change of the project's pipelines,
//! profiles, tools or connections and a run's start or end; never on a timer. A read that failed shows its failure
//! ([`states`]), no counts and *New pipeline* unavailable; the first read shows bones ([`bones`]).

mod archived;
mod bar;
mod bones;
mod edit;
mod chips;
mod flow;
mod leave;
mod list;
mod menu;
mod model;
mod panel;
mod panel_head;
mod ready;
mod rename;
mod row;
mod save;
mod settings;
mod sources;
mod states;
mod step_line;
mod steps;
mod undecided;
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
use orchester_workspace::messages::Message;

use self::{
  leave::{Hosted, Unsaved, UnsavedSignal},
  model::{Entry, Listing},
  states::Retry,
};
use crate::{
  navigation::{Navigation, guard::LeaveGuard},
  project::ShownProject,
  queries::{self, Again, Session, hub as hub_query, hub::PipelineHubRead},
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::{
    hub::agents,
    local_hub::{
      archive::{Asking, Target},
      head,
      leave::Leave,
    },
  },
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    failure::LastFailure,
    icon::Icon,
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
  /// The pipeline whose panel has changes that are not saved, which its row says.
  unsaved: UnsavedSignal,
  /// Where the person pressed a row while changes are not saved: it waits for their answer ([`leave`]).
  leaving: Signal<Option<Route>>,
  /// The question that asks it.
  asking: Signal<bool>,
  /// Holds every other departure (Back, a tab, the sidebar) while a panel's changes are not saved.
  guard: LeaveGuard,
  /// The pipeline whose archive confirmation shows over the page, which outlives the pipeline's panel.
  archiving: Asking,
  /// The pipeline the person pressed *Archive* on while its changes are not saved: it waits for their answer.
  archive_waits: Signal<Option<Target>>,
  /// Whether the open panel's save is under way.
  saving: Signal<bool>,
}

impl Component for PipelineHubScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let unsaved: UnsavedSignal = ctx.signal(Unsaved::default());
    let edits = unsaved.clone();
    let this = Self {
      retry: ctx.signal(0),
      again: Again::default(),
      failure: LastFailure::default(),
      unsaved,
      leaving: ctx.signal(None),
      asking: ctx.signal(false),
      guard: Navigation::of(ctx).guard(move || edits.get_untracked().0.is_some()),
      archiving: Asking::new(ctx),
      archive_waits: ctx.signal(None),
      saving: ctx.signal(false),
    };
    this.follow_question(ctx);
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, view } = ctx.props::<Props>().clone();
    let loaded = self.read(ctx, &project);
    let failed = self.failure.follow(&loaded);
    let unreadable = failed.is_some();
    let reading = loaded.is_pending();
    let hub = loaded.data.as_ref().filter(|_| failed.is_none());
    let listing = hub.map(|read| Listing::of(&read.hub));
    let default = hub.and_then(|read| read.default);
    let navigation = Navigation::of(ctx);
    let leave = Leave::new(&navigation, &self.leaving, self.unsaved.get().0.is_some());
    let selected = listing.as_ref().and_then(|listing| listing.selected(view)).cloned();
    let id = project.open.project.id();
    let body = match (failed, listing.as_ref()) {
      (Some((error, retrying)), _) => self.failure_page(ctx, &error, retrying),
      (None, Some(listing)) => {
        if !reading {
          keep_route_on_shown(id, view, selected.as_ref(), &navigation);
        }
        hub_body(
          ctx,
          &project,
          (listing, view),
          (selected.as_ref(), &leave, reading),
          (default, self.hosted()),
        )
      }
      (None, None) => bones::body(ctx, ctx.t("pipeline_hub.reading")),
    };
    let new = if unreadable {
      states::new_unavailable(ctx)
    } else {
      new_button(ctx, ("pipeline-hub-new", ButtonKind::Primary), &leave, id)
    };
    let leaving = listing.as_ref().and_then(|listing| {
      leave::question(
        ctx,
        (&self.leaving, &self.asking),
        (listing, &self.hosted()),
        (&navigation, &self.guard),
      )
    });
    let over: Vec<Element> = leaving.into_iter().chain(self.archiving.question(ctx)).collect();
    agents::frame(
      ctx,
      agents::Frame {
        prefix: "pipeline-hub",
        title: "pipeline_hub.title",
        current: view.filter.name(),
        status: head::status(ctx),
        new: Some(new),
        tabs: tabs(id, listing.as_ref(), selected.as_ref().map(|entry| entry.id), &leave),
        notices: Vec::new(),
        body,
        over,
      },
    )
  }
}

impl PipelineHubScreen {
  /// The question follows what waits for an answer, and what waits follows the question closing; each writes only when
  /// it differs, or they would write each other for ever.
  fn follow_question(&self, ctx: &mut Ctx) {
    let asking = self.asking.clone();
    ctx.watch(&self.leaving, move |route| {
      if asking.get_untracked() != route.is_some() {
        asking.set(route.is_some());
      }
    });
    let asking = self.asking.clone();
    ctx.watch(&self.guard.attempt(), move |attempt| {
      if asking.get_untracked() != attempt.is_some() {
        asking.set(attempt.is_some());
      }
    });
    let asking = self.asking.clone();
    ctx.watch(&self.archive_waits, move |target| {
      if asking.get_untracked() != target.is_some() {
        asking.set(target.is_some());
      }
    });
    let (asking, unsaved) = (self.asking.clone(), self.unsaved.clone());
    ctx.watch(&self.saving, move |saving| {
      // A question that waited for a save which stored the edit has nothing left to ask.
      if !*saving && unsaved.get_untracked().0.is_none() && asking.get_untracked() {
        asking.set(false);
      }
    });
    let (leaving, waits, guard) = (self.leaving.clone(), self.archive_waits.clone(), self.guard.clone());
    ctx.watch(&self.asking, move |open| {
      if *open {
        return;
      }
      if leaving.get_untracked().is_some() {
        leaving.set(None);
      }
      if waits.get_untracked().is_some() {
        waits.set(None);
      }
      if guard.attempt().get_untracked().is_some() {
        guard.keep_editing();
      }
    });
  }

  /// What the panel is given of what the screen hosts: where the list hears of its changes and the confirmations that
  /// outlive it.
  fn hosted(&self) -> Hosted {
    Hosted {
      unsaved: self.unsaved.clone(),
      archiving: self.archiving.clone(),
      archive_waits: self.archive_waits.clone(),
      saving: self.saving.clone(),
    }
  }

  /// The pipelines as the project's database has them, from the session's cache: the last answer at once, read again
  /// behind it on each visit, when a pipeline, a profile, a tool or a connection changed and when a run changes state
  /// (`queries::changes`); and on *Try again*.
  fn read(&self, ctx: &mut Ctx, project: &ShownProject) -> FutureState<PipelineHubRead, Message> {
    let session = Session::of(ctx);
    let query = ctx.query(hub_query::pipeline_hub(session, project.open.repo_root.clone()));
    self.again.follow(self.retry.get(), &query);
    queries::reading(&query)
  }

  fn failure_page(&self, ctx: &Ctx, error: &Message, retrying: bool) -> Element {
    let count = self.retry.clone();
    let retry = Retry {
      retrying,
      press: Arc::new(move || count.update(|n| *n += 1)),
    };
    states::failure(ctx, error, retry)
  }
}

/// The list and the panel, or what stands in for them when the tab holds nothing. `reading`: they show the last answer
/// while it is read again.
fn hub_body(
  ctx: &mut Ctx,
  project: &ShownProject,
  (listing, view): (&Listing, PipelinesView),
  (selected, leave, reading): (Option<&Entry>, &Leave, bool),
  (default, hosted): (Option<ResourceId>, Hosted),
) -> Element {
  let id = project.open.project.id();
  let Some(entry) = selected else {
    return empty(ctx, view.filter, leave, id);
  };
  let now = Timestamp::now();
  let shown = list::Shown {
    project: id,
    filter: view.filter,
    selected: Some(entry.id),
    leave,
    unsaved: hosted.unsaved.get(),
    now,
    default,
  };
  let list = list::list(ctx, listing, &shown);
  let panel = panel::panel(
    ctx,
    panel::Props {
      project: project.clone(),
      entry: entry.clone(),
      assignables: listing.assignables().clone(),
      default: default == Some(entry.id),
      hosted,
    },
  );
  agents::master_detail("pipeline-hub", reading, (list, panel))
}

/// *New pipeline*, which opens the page that makes one: the head's is the primary button, the empty body's the
/// secondary.
fn new_button(ctx: &Ctx, (id, kind): (&str, ButtonKind), leave: &Leave, project: ProjectId) -> Element {
  ActionButton::new(id.to_owned(), kind, ctx.t("pipeline_hub.new"))
    .icon(Icon::Plus)
    .press(leave.press(Route::Project(project, ProjectPage::NewPipeline)))
    .into()
}

/// The open tab holds nothing: what a pipeline is, with *New pipeline* when the project has none.
fn empty(ctx: &Ctx, filter: PipelinesFilter, leave: &Leave, project: ProjectId) -> Element {
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
      new_button(ctx, ("pipeline-hub-empty-new", ButtonKind::Secondary), leave, project),
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
  leave: &Leave,
) -> Vec<agents::Tab> {
  PipelinesFilter::ALL
    .into_iter()
    .map(|choice| {
      let item = selected.filter(|item| listing.is_some_and(|listing| listing.holds(choice, *item)));
      let route = Route::Project(project, ProjectPage::pipelines(PipelinesView::new(choice, item)));
      let leave = leave.clone();
      agents::Tab {
        name: choice.name(),
        label: label_key(choice),
        count: agents::TabCount::of(listing.map(|listing| listing.count(choice))),
        open: Arc::new(move || leave.go(route.clone())),
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
