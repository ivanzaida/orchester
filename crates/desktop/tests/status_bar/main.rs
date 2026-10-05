//! The status bar (`pmOpt`) rendered headlessly: counts match the Overview, a long failure
//! stays before the right group while its full text remains in the hint, and the MCP item shows
//! the port Orchester's own MCP server bound, or that it did not start, and its hint where lurq's
//! server for the window listens, never a token.

use std::{
  path::PathBuf,
  time::{Instant, SystemTime},
};

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  mcp::McpConfig,
  node::{Element, element::ElementRef},
};
use orchester_desktop::{
  activity::{Activity, ActivityLog, OperationSpec},
  app_mcp::AppMcpStatus,
  embedded_mcp::McpStatus,
  i18n,
  messages::Message,
  runner::Runners,
  status_bar::{ProjectBar, StatusBar, StatusBarProps},
  tabs::feed::WindowFeed,
  theme,
};
use orchester_ipc::SessionEnd;
use tokio::runtime::Runtime;

mod approvals;
mod narrow;
mod plan_limits;

/// Provides the session's activity and the window's feed of it, as the root does, above the bar.
struct Shell;

#[derive(Clone, PartialEq)]
struct ShellProps {
  readings: orchester_desktop::plan_limits::Readings,
  bar: StatusBarProps,
  log: ActivityLog,
  mcp: McpStatus,
  app: AppMcpStatus,
}

impl lurq::app::component::DevtoolsInspectable for ShellProps {}

impl Component for Shell {
  type Props = ShellProps;

  fn create(ctx: &mut Ctx) -> Self {
    let ShellProps { mcp, app, .. } = ctx.props::<ShellProps>().clone();
    ctx.provide(mcp);
    ctx.provide(app);
    let activity = Activity::new();
    ctx.provide(WindowFeed::new(&activity));
    ctx.provide(activity);
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ShellProps>().clone();
    WindowFeed::of(ctx).activity.set(props.log);
    WindowFeed::of(ctx).limits.set(props.readings);
    ctx.mount_keyed::<StatusBar>("status-bar", props.bar)
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
      project_name: "Kontur".into(),
      connections: orchester_desktop::plan_limits::Connections::Ready(Vec::new()),
      repo_root: PathBuf::from("kontur"),
      runner: runners.status(),
      queued: Some(0),
      needs_you,
      usage: None,
      lost_to_runner: Some(0),
      approvals: Default::default(),
      dialog: Default::default(),
      auto_approving: Default::default(),
    }),
  }
}

fn laid_out(props: StatusBarProps) -> (App, Tree) {
  laid_out_with_log(props, ActivityLog::default())
}

fn laid_out_with_log(props: StatusBarProps, log: ActivityLog) -> (App, Tree) {
  laid_out_with(props, log, (McpStatus::Unavailable, AppMcpStatus::Unavailable))
}

fn laid_out_with(props: StatusBarProps, log: ActivityLog, (mcp, server): (McpStatus, AppMcpStatus)) -> (App, Tree) {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  tree.resize(1440, 28);
  tree.mount_root::<Shell>(
    &mut app,
    ShellProps {
      bar: props,
      log,
      mcp,
      app: server,
      readings: Default::default(),
    },
  );
  tree.pass_headless(&mut app);
  (app, tree)
}

fn bounds(tree: &mut Tree, id: &str) -> lurq::core::ElementRect {
  tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .expect("item is laid out")
}

#[test]
fn status_bar_long_failure_stays_before_counts_and_has_full_hint() {
  let mut log = ActivityLog::default();
  let long_reason = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(32);
  let spec = OperationSpec::new(
    Message::new("statusBar.operations.projectCreate.running"),
    Message::new("statusBar.operations.projectCreate.done"),
    Message::new("statusBar.operations.projectCreate.failed").with("name", long_reason.clone()),
  );
  let id = log.start(spec, Instant::now());
  log.fail(id, Message::new("errors.unsupportedPath"), SystemTime::now());
  let (mut app, mut tree) = laid_out_with_log(StatusBarProps { project: None }, log);
  let activity = bounds(&mut tree, "status-activity");
  let version = bounds(&mut tree, "status-version");
  assert!(
    activity.x + activity.width <= version.x,
    "failure item overlaps the right-hand group"
  );
  hover(&mut app, &mut tree, "status-activity");
  let hint = bounds(&mut tree, "status-bar-hint");
  let hint_text = bounds(&mut tree, "status-bar-hint-text");
  assert!(
    hint.width <= 420.0,
    "the hint wraps instead of growing beyond the window"
  );
  assert!(hint_text.height > 30.0, "the complete failure wraps onto several lines");
  assert!(
    hint_text.x + hint_text.width <= hint.x + hint.width,
    "hint text stays inside its panel"
  );
  assert!(
    item_texts(&mut tree, "status-bar-hint")
      .join(" ")
      .contains(&long_reason)
  );
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

/// Every text in the element `id` and below it, icon glyphs included.
fn all_texts(tree: &mut Tree, id: &str) -> String {
  let mut all = Vec::new();
  texts(tree.get_element_by_id(id).expect(id), &mut all);
  all.join(" ")
}

#[test]
fn status_bar_mcp_shows_the_app_server_port_and_never_a_token() {
  let mut server = Tree::new();
  let handle = server.enable_mcp(McpConfig::new());
  let token = handle.token().to_owned();
  let window = McpStatus::of(&handle);
  let McpStatus::Listening { port: window_port } = window else {
    server.shutdown_mcp();
    panic!("the server binds a free port");
  };
  server.shutdown_mcp();
  let port = std::num::NonZeroU16::new(47291).expect("port");
  let app = AppMcpStatus::Listening { port };

  let (mut app_state, mut tree) = laid_out_with(project(Some(0)), ActivityLog::default(), (window, app));
  assert_eq!(item_texts(&mut tree, "status-mcp"), ["MCP", "127.0.0.1:47291"]);
  let (mcp, version) = (bounds(&mut tree, "status-mcp"), bounds(&mut tree, "status-version"));
  assert!(mcp.x + mcp.width <= version.x, "the MCP item sits before the version");
  hover(&mut app_state, &mut tree, "status-mcp");
  let hint = all_texts(&mut tree, "status-bar-hint");
  assert!(hint.contains("http://127.0.0.1:47291/mcp"), "{hint}");
  assert!(
    hint.contains(&format!("orchester-mcp/{}.json", std::process::id())),
    "{hint}"
  );
  assert!(hint.contains(&format!("http://127.0.0.1:{window_port}/mcp")), "{hint}");
  let shown = format!("{} {hint}", all_texts(&mut tree, "status-bar"));
  assert!(!shown.contains(&token), "the bearer token is never shown");
}

#[test]
fn status_bar_mcp_unavailable_when_the_app_server_did_not_start() {
  let (mut app, mut tree) = laid_out_with(
    StatusBarProps { project: None },
    ActivityLog::default(),
    (McpStatus::Unavailable, AppMcpStatus::Unavailable),
  );
  assert_eq!(item_texts(&mut tree, "status-mcp"), ["MCP unavailable"]);
  hover(&mut app, &mut tree, "status-mcp");
  assert_eq!(
    item_texts(&mut tree, "status-bar-hint"),
    ["Orchester’s MCP server did not start, so automation cannot control the app. The app log has the reason."]
  );
}
