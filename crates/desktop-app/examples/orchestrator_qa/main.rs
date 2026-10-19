//! The Orchestrator chat (`Orchestrator` `TrG83`) and its settings (`Orchestrator settings` `Xir64`), for design
//! verification: `cargo build -p orchester-runner --features test-agent --bins` once, then
//! `cargo run -p orchester-app --example orchestrator_qa -- <state>`, `state` one of [`State::ALL`].
//!
//! Each start seeds a new folder under `.tmp/qa/orchestrator/` ([`data`]): the seed's `tasks` project, a Claude Code
//! connection whose program is `orchester-fake-claude` and a Codex one whose program is `orchester-fake-codex`, both
//! from this build's target folder (stand-ins: no real CLI runs and no sign-in is read), and conversations in each
//! state. The project's runner is this build's, so a message sent
//! in the window is answered by the stand-in: `Keep thinking …` keeps it thinking (Stop response), `Please sign in`
//! fails as a CLI that is not signed in, `Fail now` fails with an error, `Call <tool> <JSON>` calls one of the chat's
//! domain tools (e.g. `Call create_task {"project":"Kontur","title":"Delivery log"}`), anything else is answered
//! with `Noted: …`. The verifier connects to the window's QA server through the discovery file in
//! `.tmp/qa/orchestrator/mcp/` (`tools/lurq-mcp-client.py --discovery .tmp/qa/orchestrator/mcp`); the window can be
//! driven, and `hold_reads` holds its reads to capture a conversation loading. `ORCHESTER_APP_WINDOW_POSITION=X,Y`
//! opens the window there, as for the app (`startup::position`).

#[path = "../seed/kontur/mod.rs"]
mod kontur;

mod applied;
mod changes;
mod continuation;
mod data;
mod links;
mod proposals;
mod refused;
#[path = "../../tests/support/signed_in.rs"]
mod signed_in;

use std::{
  env,
  error::Error,
  io::Write as _,
  path::{Path, PathBuf},
  sync::{Arc, OnceLock},
};

use lurq::app::{App, Tree, WindowCornerRadius, wgpu_render::WgpuRenderEngine, winit_shell::WinitWindow};
use orchester_app::{
  appearance::{Appearance, ThemeSetting},
  brand, i18n,
  mcp::{self, AppMcp, McpLink, McpMode, McpSettings, WindowLink},
  root::{Root, RootProps, Start},
  routes::{OrchestratorChat, ProjectPage, Route},
  startup::WindowPosition,
  theme::{self, Mode},
  ui::loader,
};
use orchester_core::id::ChatId;
use tokio::runtime::Runtime;

/// Where the window opens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
  /// `HvzWI`: a new conversation with its suggestions.
  New,
  /// `ZJzV0`: a conversation with replies, a task created as a draft and a start refused (`nQIhx`, `kDOn9` *Chat
  /// can't bypass a rule*). Sending `Keep thinking …` shows `kDOn9` *Delivered · thinking · Stop response*.
  Conversation,
  /// `kDOn9` *Response stopped*.
  Stopped,
  /// `kDOn9` *Provider access failure*.
  Failed,
  /// A turn cut short when the runner before ended.
  Interrupted,
  /// A start the agent proposed, of a task edited since: a person's *Start* is refused as changed since proposed
  /// (synthetic input gets the person-only note instead).
  ProposalChanged,
  /// That proposal after a person's *Start* ([`refused`]): its card shows the refusal.
  ProposalRefused,
  /// Publish, cancel and merge proposals waiting for the person, after earlier ones applied, refused and dismissed
  /// ([`changes`]).
  Changes,
  /// `kPs8W`: the latest conversation; press *History* to open the panel.
  History,
  /// `qAq0z` (chat side): nothing chosen for the Orchestrator yet, a new conversation that can't send.
  NotReady,
  /// `kDOn9` *Failed send · draft kept*: the runner's program is missing; send a message to see it.
  NoRunner,
  /// `jmyWu` (Profile, Connection & model only): the Orchestrator's settings.
  Settings,
  /// `qAq0z` *No connection can run the Orchestrator*: only the seed's Codex connection, which says why it can't.
  SettingsNoConnection,
  /// `w262B`, `agv8r`: replies with links in Kontur and in Atlas, to the web, and to addresses shown as text.
  Links,
  /// One more retry of the seed's run that waits for a decision, proposed by the chat and waiting for the person
  /// ([`continuation`]; no Pencil frame yet).
  Continue,
  /// That retry after the person's *Allow retry*: the runner retries the step, and the run's end lands in the
  /// conversation.
  ContinueApplied,
  /// `ExRt1`: proposals of turns a wait or a run's end started by itself, in each state ([`proposals`]).
  Proposals,
  /// `ExRt1` `gOzsF`: messages that move a run, proposed and applied, among them one that ran a task again
  /// ([`applied`]).
  MessagesApplied,
}

impl State {
  const ALL: [(&'static str, Self); 18] = [
    ("new", Self::New),
    ("conversation", Self::Conversation),
    ("stopped", Self::Stopped),
    ("failed", Self::Failed),
    ("interrupted", Self::Interrupted),
    ("proposal-changed", Self::ProposalChanged),
    ("proposal-refused", Self::ProposalRefused),
    ("changes", Self::Changes),
    ("history", Self::History),
    ("not-ready", Self::NotReady),
    ("no-runner", Self::NoRunner),
    ("settings", Self::Settings),
    ("settings-no-connection", Self::SettingsNoConnection),
    ("links", Self::Links),
    ("continue", Self::Continue),
    ("continue-applied", Self::ContinueApplied),
    ("proposals", Self::Proposals),
    ("messages-applied", Self::MessagesApplied),
  ];

  fn named(name: &str) -> Option<Self> {
    Self::ALL
      .iter()
      .find(|(known, _)| *known == name)
      .map(|(_, state)| *state)
  }
}

fn main() -> Result<(), Box<dyn Error>> {
  let name = env::args().nth(1).unwrap_or_else(|| "new".to_owned());
  let state = State::named(&name).ok_or_else(|| {
    let names: Vec<&str> = State::ALL.iter().map(|(name, _)| *name).collect();
    format!("expected one of {}", names.join(", "))
  })?;
  let target = env::current_exe()?
    .parent()
    .and_then(Path::parent)
    .ok_or("the example's folder is in the target folder")?
    .to_path_buf();
  let program = |name: &str| target.join(format!("{name}{}", env::consts::EXE_SUFFIX));
  let fakes = (program("orchester-fake-claude"), program("orchester-fake-codex"));
  if !fakes.0.is_file() || !fakes.1.is_file() {
    return Err("build the stand-ins first: cargo build -p orchester-runner --features test-agent --bins".into());
  }
  let scratch = qa_folder()?;
  std::fs::create_dir_all(&scratch)?;
  let root = tempfile::Builder::new().prefix("app-").tempdir_in(&scratch)?;
  let chosen = !matches!(state, State::NotReady | State::SettingsNoConnection);
  let prepared = data::prepare(root.path(), fakes, (chosen, state != State::SettingsNoConnection))?;
  if state == State::Changes {
    // A profile draft that widens is proposed while the person lets the Orchestrator publish drafts: its card says so.
    orchester_store::AppDb::open(&prepared.database)?.set_orchestrator_publishes_drafts(true)?;
  }
  let runner = if state == State::NoRunner {
    root.path().join("no-runner").join("orchester-runner")
  } else {
    program("orchester-runner")
  };
  writeln!(
    std::io::stdout().lock(),
    "mcp {}\ndata {}\nseeded {}",
    scratch.join("mcp").display(),
    root.path().display(),
    prepared.shown
  )?;
  window(&prepared, (state, runner), &scratch)
}

/// `.tmp/qa/orchestrator/` of this checkout.
fn qa_folder() -> Result<PathBuf, &'static str> {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .ancestors()
    .nth(2)
    .map(|checkout| checkout.join(".tmp").join("qa").join("orchestrator"))
    .ok_or("the crate is two folders into the checkout")
}

/// The route `state` opens on; `chat` the conversation a state seeded once the session started.
fn route(prepared: &data::Prepared, state: State, chat: Option<ChatId>) -> Route {
  if let Some(chat) = chat {
    return Route::Project(
      prepared.project,
      ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)),
    );
  }
  let chats = &prepared.conversations;
  let page = match state {
    State::New | State::NotReady | State::NoRunner => ProjectPage::Orchestrator(OrchestratorChat::New),
    State::Conversation => ProjectPage::Orchestrator(OrchestratorChat::Chat(chats.answered)),
    State::Stopped => ProjectPage::Orchestrator(OrchestratorChat::Chat(chats.stopped)),
    State::Failed => ProjectPage::Orchestrator(OrchestratorChat::Chat(chats.failed)),
    State::Interrupted => ProjectPage::Orchestrator(OrchestratorChat::Chat(chats.interrupted)),
    State::ProposalChanged | State::ProposalRefused => {
      ProjectPage::Orchestrator(OrchestratorChat::Chat(chats.proposal_changed))
    }
    State::Changes => ProjectPage::Orchestrator(OrchestratorChat::Chat(chats.changes)),
    State::History => ProjectPage::Orchestrator(OrchestratorChat::Latest),
    State::Links => ProjectPage::Orchestrator(OrchestratorChat::Chat(prepared.links)),
    State::Settings | State::SettingsNoConnection => ProjectPage::OrchestratorSettings,
    State::Continue | State::ContinueApplied | State::Proposals | State::MessagesApplied => {
      ProjectPage::Orchestrator(OrchestratorChat::Latest)
    }
  };
  Route::Project(prepared.project, page)
}

/// The conversation `state` seeds once the session started, with its `services`, in the words of `i18n`: through
/// the chat's tools and the window's Apply, or as the chat keeps its actions; `None` for a state that seeds none.
fn seeded_chat(
  state: State,
  (runtime, services, i18n): (&Runtime, &orchester_app::services::Services, &lurq::app::i18n::I18n),
  prepared: &data::Prepared,
) -> Result<Option<ChatId>, Box<dyn Error>> {
  Ok(Some(match state {
    State::Continue | State::ContinueApplied => continuation::conversation(
      runtime,
      (services, i18n.clone()),
      prepared,
      state == State::ContinueApplied,
    )?,
    State::Proposals => proposals::conversation(prepared, i18n)?,
    State::MessagesApplied => applied::conversation(runtime, (services, i18n.clone()), prepared)?,
    _ => return Ok(None),
  }))
}

/// The window on the seeded data at `state`, its project's runner `runner`; returns once it closed.
fn window(prepared: &data::Prepared, (state, runner): (State, PathBuf), scratch: &Path) -> Result<(), Box<dyn Error>> {
  let runtime = Runtime::new()?;
  // Past the sign-in gate, as a person who signed in; the stand-in cloud answers while the window is open.
  let cloud = signed_in::account_cloud::AccountCloud::start();
  let launch = signed_in::launch(&prepared.database, runner, runtime.handle(), &cloud);
  let appearance = Appearance::new(ThemeSetting::Dark, Arc::new(OnceLock::new()), None);
  let mut app = App::new();
  app.set_tokio_handle(runtime.handle().clone());
  theme::install(&mut app, Mode::Dark);
  i18n::install(&app)?;
  let chat = match launch.services() {
    Some(services) => seeded_chat(state, (&runtime, services, app.i18n()), prepared)?,
    None => None,
  };
  let start = match launch.start() {
    Start::Ready(mut ready) => {
      ready.landing.route = route(prepared, state, chat);
      Start::Ready(ready)
    }
    problem @ Start::Problem(_) => problem,
  };
  if let (State::ProposalRefused, Some(services)) = (state, launch.services()) {
    refused::start(
      &runtime,
      (services, app.i18n().clone()),
      prepared,
      prepared.conversations.proposal_changed,
    )?;
  }
  let mut tree = Tree::new();
  tree.set_render_engine_factory(|| Box::new(WgpuRenderEngine::new()));
  loader::install(&mut tree);
  let link = WindowLink::default();
  let settings = McpSettings::with_window_control(true);
  let config = mcp::config(settings, &link, &appearance, launch.services(), app.i18n());
  let server = AppMcp::start(
    &mut tree,
    runtime.handle(),
    McpMode::Qa(scratch.join("mcp")),
    (config, settings),
    &link.input_gate(),
  );
  tree.mount_root::<Root>(
    &mut app,
    RootProps {
      link,
      mcp: McpLink::new(&server),
      start,
    },
  );
  let window = WinitWindow::new(app, tree)
    .with_title("Orchester Orchestrator QA")
    .with_icon(brand::window_icon()?)
    .with_size(1440, 900)
    .with_min_size(960, 640)
    .with_start_without_focus(true)
    .with_decorations(false)
    .with_corner_radius(WindowCornerRadius::Rounded);
  // Where QA asks for it (`ORCHESTER_APP_WINDOW_POSITION`, as the app reads it), e.g. off the person's screen.
  match WindowPosition::from_environment()? {
    Some(WindowPosition { x, y }) => window.with_position(x, y),
    None => window,
  }
  .run();
  server.shutdown();
  launch.end();
  Ok(())
}
