//! The hub's tabs with their counts (`Tabs` `Cc3JP`): Profiles, Instructions, Skills, Tools and Memory, each with how
//! many it holds once counted; a tab that holds none shows no count (06, 06b). Memory counts the proposals that wait
//! for approval, in the warning tone. The counts are read off the UI thread whenever the project's resources,
//! tasks or runs change; the last counts show meanwhile, on every tab of the hub.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_workspace::{agents::hub::HubCounts, background, messages, tasks::outside::Outside};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  services::Services,
  ui::tabs::{PageTab, page_tabs},
};

/// A tab of the hub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HubTab {
  Profiles,
  Instructions,
  Skills,
  Tools,
  Memory,
}

impl HubTab {
  pub(crate) const ALL: [Self; 5] = [Self::Profiles, Self::Instructions, Self::Skills, Self::Tools, Self::Memory];

  /// The tab's name in its element id and the hub's `tab` description.
  pub(crate) const fn name(self) -> &'static str {
    match self {
      Self::Profiles => "profiles",
      Self::Instructions => "instructions",
      Self::Skills => "skills",
      Self::Tools => "tools",
      Self::Memory => "memory",
    }
  }

  pub(crate) const fn label_key(self) -> &'static str {
    match self {
      Self::Profiles => "hub.tabs.profiles",
      Self::Instructions => "hub.tabs.instructions",
      Self::Skills => "hub.tabs.skills",
      Self::Tools => "hub.tabs.tools",
      Self::Memory => "hub.tabs.memory",
    }
  }

  /// How many the tab holds of `counts`; whether that counts what waits for the person.
  const fn count(self, counts: &HubCounts) -> (usize, bool) {
    match self {
      Self::Profiles => (counts.profiles, false),
      Self::Instructions => (counts.instructions, false),
      Self::Skills => (counts.skills, false),
      Self::Tools => (counts.tools, false),
      Self::Memory => (counts.waiting, true),
    }
  }
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  tab: HubTab,
}

impl DevtoolsInspectable for Props {}

/// The tabs of `project`'s hub with `tab` open.
pub(super) fn strip(ctx: &mut Ctx, project: &ShownProject, tab: HubTab) -> Element {
  ctx.mount_keyed::<HubTabs>(
    &format!("agents-hub-tabs-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      tab,
    },
  )
}

struct HubTabs;

impl Component for HubTabs {
  type Props = Props;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project, tab } = ctx.props::<Props>().clone();
    let feed = &project.feed;
    let outside = feed.outside.get();
    let changed = [
      Outside::Profiles,
      Outside::Instructions,
      Outside::Skills,
      Outside::Tools,
    ]
    .into_iter()
    .map(|kind| outside.of(kind))
    .sum::<u64>();
    let deps = (
      project.open.repo_root.clone(),
      changed,
      feed.revision.get(),
      feed.states.get(),
    );
    let hold = Services::of(ctx).hold;
    let read = ctx
      .future(deps, move |(root, ..)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || HubCounts::load_in(&root))
            .await?
            .map_err(|error| {
              tracing::error!(%error, "cannot count the Agents hub's tabs");
              messages::store_error(&error)
            })
        }
      })
      .state()
      .get();
    if let Some(counts) = read.data.filter(|_| !read.is_pending())
      && feed.hub_counts.get_untracked() != Some(counts)
    {
      feed.hub_counts.set(Some(counts));
    }
    let counts = feed.hub_counts.get();
    let navigation = Navigation::of(ctx);
    let id = project.open.project.id();
    let tabs: Vec<PageTab> = HubTab::ALL
      .into_iter()
      .map(|choice| {
        let label = ctx.t(choice.label_key());
        let tab_id = format!("agents-hub-tab-{}", choice.name());
        let open = PageTab::to(tab_id, label, choice == tab, navigation.to(choice.route(id)));
        match counts.map(|counts| choice.count(&counts)) {
          Some((count, waits)) if count > 0 => open.count(count, waits),
          _ => open,
        }
      })
      .collect();
    page_tabs(ctx, "agents-hub-tabs", tabs)
  }
}
