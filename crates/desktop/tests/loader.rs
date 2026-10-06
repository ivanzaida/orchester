//! Loader icons turn while they are shown and ask for no frames otherwise. The close dialog's rows
//! of running work (`clVjO`) hold real loaders; the dialog is mounted on stage and offstage the way
//! a window tab's content is. The fixtures are test data only.

use std::{f32::consts::TAU, sync::Arc, thread, time::Duration};

use lurq::{
  app::{App, Tree, component::Component, ctx::Ctx},
  components::Column,
  layout::quad::QuadContent,
  node::Element,
};
use orchester_core::{connection::AgentProvider, id::RunId};
use orchester_desktop::{
  close_dialog::{CloseActions, CloseDialog, CloseDialogProps, CloseStage},
  i18n, loader,
  runner::{ActiveWork, ActiveWorkFields},
  theme,
};

/// The loader's glyph in the bundled lucide font (`Icon::Loader`).
const LOADER: &str = "\u{e109}";
/// Passes run while counting the frames a window is asked for.
const PASSES: usize = 3;

#[derive(Clone, Copy, PartialEq)]
enum Shows {
  Nothing,
  OnStage,
  Offstage,
}

#[derive(Clone, PartialEq)]
struct HostProps {
  shows: Shows,
  dialog: CloseDialogProps,
}

impl lurq::app::component::DevtoolsInspectable for HostProps {}

/// Mounts the dialog as the tab host mounts a tab's content: retained, and offstage unless shown.
struct Host;

impl Component for Host {
  type Props = HostProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<HostProps>().clone();
    let column = Column::new();
    match props.shows {
      Shows::Nothing => column,
      Shows::OnStage | Shows::Offstage => {
        column.child(ctx.mount_keyed_offstage::<CloseDialog>("dialog", props.dialog, props.shows == Shows::OnStage))
      }
    }
  }
}

fn work(task_key: &str, step: &str) -> ActiveWork {
  ActiveWork::new(&ActiveWorkFields {
    project: "Kontur",
    run: RunId::generate(),
    task_key,
    task_title: "Fix canvas zoom on trackpad",
    step,
    agent: "Builder",
    provider: AgentProvider::CodexCli,
    model: "gpt-5.5",
  })
}

fn props(shows: Shows) -> HostProps {
  let nothing: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
  HostProps {
    shows,
    dialog: CloseDialogProps {
      stage: CloseStage::Confirm,
      work: vec![work("ORC-125", "Implement"), work("ORC-127", "Run checks")].into(),
      saves: Vec::new().into(),
      actions: CloseActions {
        wait_for_agents: nothing.clone(),
        quit_now: nothing.clone(),
        cancel: nothing,
      },
    },
  }
}

/// The window's tree with the loader installed, as `main` sets it up.
fn mounted(shows: Shows) -> (App, Tree) {
  let mut app = App::new();
  theme::install(&mut app);
  i18n::install(&app).expect("bundled locales load");
  let mut tree = Tree::new();
  loader::install(&mut tree);
  tree.resize(1440, 960);
  tree.mount_root::<Host>(&mut app, props(shows));
  tree.pass_headless(&mut app);
  (app, tree)
}

/// Each loader glyph laid out: its centre and size on screen (x, y, width, height) and how far it
/// has turned, in radians. A turned quad keeps its turned top-left corner in `x`, `y`.
fn loaders(tree: &Tree) -> Vec<([f32; 4], f32)> {
  let layout = tree.last_layout().expect("the window is laid out");
  tree
    .resolve_quads(layout)
    .into_iter()
    .filter(|quad| matches!(&quad.content, QuadContent::Text { text, .. } if text == LOADER))
    .map(|quad| {
      let turn = quad.transform;
      let (half_width, half_height) = (quad.width / 2.0, quad.height / 2.0);
      let centre_x = quad.x + turn.a * half_width + turn.c * half_height;
      let centre_y = quad.y + turn.b * half_width + turn.d * half_height;
      let turned = turn.b.atan2(turn.a).rem_euclid(TAU);
      ([centre_x, centre_y, quad.width, quad.height], turned)
    })
    .collect()
}

/// How many of `PASSES` passes end with the window asked to draw another frame for a turning node.
fn frames_asked(app: &mut App, tree: &mut Tree) -> usize {
  (0..PASSES)
    .filter(|_| {
      tree.pass_headless(app);
      tree.has_active_timeline()
    })
    .count()
}

#[test]
fn loader_turns_while_shown_and_keeps_its_place_and_size() {
  let (mut app, mut tree) = mounted(Shows::OnStage);
  let start = loaders(&tree);
  assert!(!start.is_empty(), "the running rows show loaders");
  assert!(start.iter().all(|(_, turned)| *turned == 0.0), "a turn starts upright");

  let mut previous = start.clone();
  for _ in 0..2 {
    thread::sleep(Duration::from_millis(40));
    tree.pass_headless(&mut app);
    let now = loaders(&tree);
    assert_eq!(now.len(), start.len());
    for (index, (place, turned)) in now.iter().enumerate() {
      let before = previous[index].1;
      assert!(*turned != before, "the loader turned on: {before} then {turned}");
      let moved = place.iter().zip(start[index].0).map(|(now, then)| (now - then).abs());
      assert!(
        moved.fold(0.0, f32::max) < 0.01,
        "turning neither moves nor resizes the glyph: {place:?}, at first {:?}",
        start[index].0
      );
    }
    previous = now;
  }
}

#[test]
fn loader_asks_for_frames_only_while_on_stage() {
  let (mut app, mut tree) = mounted(Shows::Nothing);
  assert_eq!(frames_asked(&mut app, &mut tree), 0, "an idle window draws nothing");

  tree.update_root_props::<Host>(props(Shows::OnStage));
  assert_eq!(
    frames_asked(&mut app, &mut tree),
    PASSES,
    "a shown loader turns every frame"
  );

  tree.update_root_props::<Host>(props(Shows::Offstage));
  assert_eq!(
    frames_asked(&mut app, &mut tree),
    0,
    "an offstage tab's loader asks for no frames"
  );

  tree.update_root_props::<Host>(props(Shows::OnStage));
  assert_eq!(
    frames_asked(&mut app, &mut tree),
    PASSES,
    "back on stage it turns again"
  );

  tree.update_root_props::<Host>(props(Shows::Nothing));
  assert_eq!(
    frames_asked(&mut app, &mut tree),
    0,
    "once the work is done the window is idle"
  );
}
