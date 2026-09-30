//! The status bar (`pmOpt`) rendered headlessly: counts match the Overview, a long failure
//! stays before the right group while its full text remains in the hint, and the MCP item shows
//! the port the embedded server bound, or that it did not start, never its token.

use std::{
  net::{TcpListener, TcpStream},
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

/// Provides the session's activity and the window's feed of it, as the root does, above the bar.
struct Shell;

#[derive(Clone, PartialEq)]
struct ShellProps {
  readings: orchester_desktop::plan_limits::Readings,
  bar: StatusBarProps,
  log: ActivityLog,
  mcp: McpStatus,
}

impl lurq::app::component::DevtoolsInspectable for ShellProps {}

impl Component for Shell {
  type Props = ShellProps;

  fn create(ctx: &mut Ctx) -> Self {
    let mcp = ctx.props::<ShellProps>().mcp;
    ctx.provide(mcp);
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
      connections: Vec::new(),
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
  laid_out_with_log(props, ActivityLog::default())
}

fn laid_out_with_log(props: StatusBarProps, log: ActivityLog) -> (App, Tree) {
  laid_out_with(props, log, McpStatus::Unavailable)
}

fn laid_out_with(props: StatusBarProps, log: ActivityLog, mcp: McpStatus) -> (App, Tree) {
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

fn cli(provider: orchester_core::connection::AgentProvider) -> orchester_core::connection::AgentConnection {
  use orchester_core::{connection::AgentConnection, id::ConnectionId, name::Name, time::Timestamp};
  AgentConnection::new(
    ConnectionId::generate(),
    Name::new("Local agent").expect("name"),
    provider,
    std::env::current_dir()
      .expect("workspace")
      .join(provider.program_name()),
    Timestamp::now(),
  )
  .expect("connection")
}

fn limits_readings(percent: f64, extra: bool, expired: bool) -> orchester_core::connection::plan_limits::PlanReadings {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let reset = Timestamp::from_unix_millis(now.unix_millis() + if expired { -1000 } else { 7_200_000 });
  let mut readings = PlanReadings::default();
  for provider in AgentProvider::ALL {
    readings.observe(LimitReading {
      provider,
      at: now,
      limits: PlanLimits {
        windows: vec![
          LimitWindow::new(
            10080,
            if provider == AgentProvider::CodexCli {
              percent
            } else {
              100.0
            },
            Some(reset),
            LimitState::Normal,
            provider == AgentProvider::ClaudeCodeCli && extra,
          )
          .expect("window"),
        ],
        plan: None,
        credits: None,
      },
    });
  }
  readings
}

fn with_limits(
  width: u32,
  readings: orchester_core::connection::plan_limits::PlanReadings,
  connections: Vec<orchester_core::connection::AgentConnection>,
) -> (App, Tree) {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("locale");
  let mut tree = Tree::new();
  tree.resize(width, 960);
  let mut bar = project(Some(2));
  bar.project.as_mut().expect("project").connections = connections;
  tree.mount_root::<Shell>(
    &mut app,
    ShellProps {
      bar,
      log: ActivityLog::default(),
      mcp: McpStatus::Listening {
        port: std::num::NonZeroU16::new(47291).expect("port"),
      },
      readings: orchester_desktop::plan_limits::Readings(readings),
    },
  );
  for _ in 0..3 {
    tree.pass_headless(&mut app);
  }
  (app, tree)
}

#[test]
fn status_bar_plan_limits_960_preserves_limits_and_drops_labels_when_reached() {
  use orchester_core::connection::AgentProvider;
  for reached in [false, true] {
    let readings = limits_readings(if reached { 100.0 } else { 65.0 }, true, false);
    let (mut app, mut tree) = with_limits(960, readings, AgentProvider::ALL.into_iter().map(cli).collect());
    assert!(tree.get_element_by_id("status-version").is_none());
    assert!(tree.get_element_by_id("status-tokens").is_none());
    let limits = bounds(&mut tree, "status-plan-limits");
    let runner = bounds(&mut tree, "status-runner");
    let mcp = bounds(&mut tree, "status-mcp");
    assert!(runner.x + runner.width <= limits.x);
    assert!(limits.x + limits.width <= mcp.x);
    assert!(mcp.x + mcp.width <= 960.0);
    assert_eq!(
      item_texts(&mut tree, "status-agents")
        .iter()
        .any(|text| text == "Agents"),
      !reached
    );
    hover(&mut app, &mut tree, "status-plan-limits");
    let hint = bounds(&mut tree, "status-plan-limits-hint");
    assert!(hint.x >= 0.0 && hint.x + hint.width <= 960.0);
    let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
    assert!(copy.contains("Extra usage"));
    assert!(copy.contains("Not reported yet"));
    assert!(copy.contains("From the last Codex turn"));
  }
}

#[test]
fn status_bar_plan_limits_unknown_signed_out_and_expired() {
  use orchester_core::{
    connection::{AgentProvider, CheckResult, ConnectionCheck, ModelCatalog, SignIn},
    time::Timestamp,
  };
  let codex = cli(AgentProvider::CodexCli);
  let mut claude = cli(AgentProvider::ClaudeCodeCli);
  claude.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::Available {
      version: "2.1.89".into(),
      sign_in: SignIn::SignedOut,
      catalog: ModelCatalog::new(orchester_core::connection::CatalogSource::DocumentedAliases, Vec::new()),
    },
  });
  let (mut app, mut tree) = with_limits(1440, Default::default(), vec![codex, claude]);
  assert_eq!(
    item_texts(&mut tree, "status-plan-limits"),
    ["Codex", "—", "·", "Claude", "—"]
  );
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("They appear after the next Codex turn in any open project."));
  assert!(copy.contains("not signed in on this computer"));
  assert!(copy.contains("Codex Not read yet"));
  assert!(copy.contains("Claude Code Not signed in"));
  assert_eq!(copy.matches("Connection checked").count(), 1);
  assert!(bounds(&mut tree, "status-plan-limits-claude-heading").height < 20.0);
  let (mut app, mut tree) = with_limits(
    1440,
    limits_readings(100.0, false, true),
    vec![cli(AgentProvider::CodexCli)],
  );
  assert_eq!(item_texts(&mut tree, "status-plan-limits"), ["Codex", "—"]);
  hover(&mut app, &mut tree, "status-plan-limits");
  assert!(
    item_texts(&mut tree, "status-plan-limits-hint")
      .join(" ")
      .contains("read again with the next turn")
  );
}

#[test]
fn status_bar_plan_limits_absent_without_connections() {
  let (_, mut tree) = with_limits(960, Default::default(), Vec::new());
  assert!(tree.get_element_by_id("status-plan-limits").is_none());
}

#[test]
fn status_bar_plan_limits_unavailable_names_connection_and_project() {
  use orchester_core::{
    connection::{AgentProvider, CheckResult, ConnectionCheck},
    time::Timestamp,
  };
  let mut connection = cli(AgentProvider::ClaudeCodeCli);
  connection.record_check(ConnectionCheck {
    at: Timestamp::now(),
    result: CheckResult::CouldNotRun,
  });
  let (mut app, mut tree) = with_limits(1440, Default::default(), vec![connection]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("Claude Code No working connection"));
  assert!(copy.contains("Local agent · Kontur could not start Claude Code when it was checked at"));
}

#[test]
fn status_bar_plan_limits_partial_claude_keeps_window_ages() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let old = Timestamp::from_unix_millis(now.unix_millis() - 180_000);
  let mut readings = PlanReadings::default();
  for (minutes, percent, at) in [(10080, 42.0, old), (300, 12.0, now)] {
    readings.observe(LimitReading {
      provider: AgentProvider::ClaudeCodeCli,
      at,
      limits: PlanLimits {
        windows: vec![LimitWindow::new(minutes, percent, None, LimitState::Normal, false).expect("window")],
        plan: None,
        credits: None,
      },
    });
  }
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint");
  assert!(
    copy
      .iter()
      .any(|line| line.starts_with("5-hour · From the last Claude turn") && line.ends_with("Just now"))
  );
  assert!(
    copy
      .iter()
      .any(|line| line.starts_with("Weekly · From the last Claude turn") && line.ends_with("3m ago"))
  );
}

#[test]
fn status_bar_plan_limits_headers_order_and_combined_account_rows() {
  use orchester_core::{
    connection::{
      AgentProvider,
      plan_limits::{Credits, LimitReading, LimitState, LimitWindow, PlanLimits, PlanReadings},
    },
    time::Timestamp,
  };
  let now = Timestamp::now();
  let mut readings = PlanReadings::default();
  readings.observe(LimitReading {
    provider: AgentProvider::ClaudeCodeCli,
    at: now,
    limits: PlanLimits {
      windows: vec![LimitWindow::new(10080, 96.0, None, LimitState::Near, false).expect("window")],
      plan: None,
      credits: None,
    },
  });
  let (mut app, mut tree) = with_limits(1440, readings, vec![cli(AgentProvider::ClaudeCodeCli)]);
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint");
  assert_eq!(
    &copy[..5],
    [
      "Claude Code",
      "Near the weekly limit",
      "5-hour",
      "Not reported yet",
      "Weekly"
    ]
  );

  let mut readings = PlanReadings::default();
  for (provider, minutes, percent, extra, plan, credits) in [
    (
      AgentProvider::CodexCli,
      10080,
      38.0,
      false,
      Some("plus".to_owned()),
      Some(Credits {
        has_credits: true,
        unlimited: false,
        balance: Some("1,250".into()),
      }),
    ),
    (AgentProvider::ClaudeCodeCli, 300, 100.0, true, None, None),
  ] {
    readings.observe(LimitReading {
      provider,
      at: now,
      limits: PlanLimits {
        windows: vec![LimitWindow::new(minutes, percent, None, LimitState::Normal, extra).expect("window")],
        plan,
        credits,
      },
    });
  }
  let (mut app, mut tree) = with_limits(1440, readings, AgentProvider::ALL.into_iter().map(cli).collect());
  hover(&mut app, &mut tree, "status-plan-limits");
  let copy = item_texts(&mut tree, "status-plan-limits-hint").join(" ");
  assert!(copy.contains("Codex Plus plan"));
  assert!(copy.contains("Credits 1,250 left"));
  assert!(copy.contains("Extra usage In use: turns past the limit use it"));
  assert!(!copy.contains("Near the"));
  assert!(!copy.contains("Not reported yet"));
  assert!(!copy.contains("limit reached"));
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
fn status_bar_mcp_shows_the_bound_port_and_never_the_token() {
  let mut server = Tree::new();
  let handle = server.enable_mcp(McpConfig::new());
  let token = handle.token().to_owned();
  let status = McpStatus::of(&handle);
  let McpStatus::Listening { port } = status else {
    server.shutdown_mcp();
    panic!("the server binds a free port");
  };
  let reachable = TcpStream::connect(("127.0.0.1", port.get())).is_ok();
  server.shutdown_mcp();
  assert!(reachable, "the server listens on the loopback port it reports");
  assert!(!format!("{status:?}").contains(&token));

  let (mut app, mut tree) = laid_out_with(project(Some(0)), ActivityLog::default(), status);
  let address = format!("127.0.0.1:{port}");
  assert_eq!(item_texts(&mut tree, "status-mcp"), ["MCP", address.as_str()]);
  let (mcp, version) = (bounds(&mut tree, "status-mcp"), bounds(&mut tree, "status-version"));
  assert!(mcp.x + mcp.width <= version.x, "the MCP item sits before the version");
  hover(&mut app, &mut tree, "status-mcp");
  let hint = all_texts(&mut tree, "status-bar-hint");
  assert!(hint.contains(&format!("http://127.0.0.1:{port}/mcp")), "{hint}");
  assert!(
    hint.contains(&format!("lurq/mcp/{}.json", std::process::id())),
    "{hint}"
  );
  let shown = format!("{} {hint}", all_texts(&mut tree, "status-bar"));
  assert!(!shown.contains(&token), "the bearer token is never shown");
}

#[test]
fn status_bar_mcp_unavailable_when_its_port_cannot_be_bound() {
  let taken = TcpListener::bind(("127.0.0.1", 0)).expect("a free loopback port");
  let port = taken.local_addr().expect("bound address").port();
  let mut server = Tree::new();
  let status = McpStatus::of(&server.enable_mcp(McpConfig::new().port(port)));
  server.shutdown_mcp();
  assert_eq!(status, McpStatus::Unavailable, "lurq reports a failed bind as port 0");

  let (mut app, mut tree) = laid_out_with(StatusBarProps { project: None }, ActivityLog::default(), status);
  assert_eq!(item_texts(&mut tree, "status-mcp"), ["MCP unavailable"]);
  hover(&mut app, &mut tree, "status-mcp");
  assert_eq!(
    item_texts(&mut tree, "status-bar-hint"),
    ["The MCP server did not start, so agents cannot drive this window. The app log has the reason."]
  );
}
