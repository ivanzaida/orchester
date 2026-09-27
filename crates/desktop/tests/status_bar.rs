//! The status bar's counts (`doeZn` in `pmOpt`) rendered headlessly: *Needs you* shows the
//! Overview's count as a plain number, `0` included, a dash only while the tasks are read, and
//! has no "not available" hint.

use std::path::PathBuf;

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  node::{Element, element::ElementRef},
};
use orchester_desktop::{
  activity::Activity,
  i18n,
  runner::Runners,
  status_bar::{ProjectBar, StatusBar, StatusBarProps},
  theme,
};
use orchester_ipc::SessionEnd;
use tokio::runtime::Runtime;

/// Provides the session's activity, as the root does, above the bar.
struct Shell;

impl Component for Shell {
  type Props = StatusBarProps;

  fn create(ctx: &mut Ctx) -> Self {
    ctx.provide(Activity::new());
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StatusBarProps>().clone();
    ctx.mount_keyed::<StatusBar>("status-bar", props)
  }
}

/// A project whose runner was never started, with `needs_you` runs waiting for the user.
fn project(needs_you: Option<usize>) -> StatusBarProps {
  let runtime = Runtime::new().expect("tokio runtime");
  let runners = Runners::new(
    PathBuf::from("orchester-runner"),
    runtime.handle().clone(),
    Activity::new(),
    SessionEnd::Clean,
  );
  StatusBarProps {
    project: Some(ProjectBar {
      repo_root: PathBuf::from("kontur"),
      runner: runners.status(),
      queued: Some(0),
      needs_you,
      usage: None,
      lost_to_runner: Some(0),
    }),
  }
}

fn laid_out(props: StatusBarProps) -> (App, Tree) {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.resize(1440, 28);
  tree.mount_root::<Shell>(&mut app, props);
  tree.pass_headless(&mut app);
  (app, tree)
}

fn texts(element: ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    texts(child, into);
  }
}

/// The texts of the item `id`, its icon glyph left out.
fn item_texts(tree: &mut Tree, id: &str) -> Vec<String> {
  let mut all = Vec::new();
  texts(tree.get_element_by_id(id).expect(id), &mut all);
  all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
  all
}

/// Moves the pointer over the item `id`.
fn hover(app: &mut App, tree: &mut Tree, id: &str) {
  let bounds = tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"));
  tree.mouse_move(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  tree.pass_headless(app);
}

#[test]
fn status_bar_needs_you_shows_the_overview_count_as_a_plain_number() {
  for (count, shown) in [(Some(2), "2"), (Some(12), "12"), (Some(0), "0"), (None, "—")] {
    let (_app, mut tree) = laid_out(project(count));
    assert_eq!(
      item_texts(&mut tree, "status-needs-you"),
      ["Needs you", shown],
      "{count:?} waiting runs"
    );
  }
}

#[test]
fn status_bar_needs_you_has_no_not_available_hint() {
  let (mut app, mut tree) = laid_out(project(Some(0)));
  hover(&mut app, &mut tree, "status-agents");
  assert!(
    tree.get_element_by_id("status-bar-hint").is_some(),
    "an item with a hint shows it"
  );
  hover(&mut app, &mut tree, "status-needs-you");
  assert!(tree.get_element_by_id("status-bar-hint").is_none());
}
