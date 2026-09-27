//! Zooming the pipeline canvas: the zoom controls, the zoom shortcuts (which stay with a focused
//! text field), the wheel with and without the zoom modifier, fitting the pipeline into view,
//! each pipeline keeping its zoom for the session, and the published notice never hiding the
//! route notes for good.

use std::{num::NonZeroU32, path::Path};

use lurq::{
  app::{
    Tree,
    events::{MouseButton, ScrollPhase},
  },
  core::ElementRect,
  node::element::ElementRef,
};
use orchester_core::{
  id::ConnectionId,
  name::Name,
  permission::StepPermissions,
  resource::{
    AgentProfile, FailureRoute, ModelId, Pipeline, PromptText, ProviderSettings, RepositoryAccess, Slug, Step, StepKey,
    StepKind,
  },
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;

use crate::harness::Desktop;

const LEVEL: &str = "pipeline-zoom-full";
const NAME: &str = "step-name";
/// More than the editor has stops, to find one by tabbing.
const MAX_STOPS: usize = 80;

/// The starter pipeline open in the editor.
fn starter() -> Desktop {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-pipelines");
  desktop.press("nav-pipelines");
  desktop.wait_for("pipelines-create-starter");
  desktop.click("pipelines-create-starter");
  desktop.wait_for(NAME);
  desktop
}

fn text(element: ElementRef<'_>) -> Option<String> {
  element
    .text_content()
    .map(str::to_owned)
    .or_else(|| element.children().into_iter().find_map(text))
}

/// The zoom level the controls show, e.g. `100%`.
fn level(desktop: &mut Desktop) -> String {
  desktop
    .tree
    .get_element_by_id(LEVEL)
    .and_then(text)
    .expect("the zoom controls show the level")
}

/// The zoom modifier, Ctrl or Cmd on macOS, pressed or released.
fn modifier(tree: &mut Tree, down: bool) {
  let (key, code) = if cfg!(target_os = "macos") {
    ("Super", "SuperLeft")
  } else {
    ("Control", "ControlLeft")
  };
  let (ctrl, meta) = (down && !cfg!(target_os = "macos"), down && cfg!(target_os = "macos"));
  if down {
    tree.key_down_with_meta(key.into(), code.into(), false, ctrl, false, meta);
  } else {
    tree.key_up_with_meta(key.into(), code.into(), false, ctrl, false, meta);
  }
}

/// A shortcut with the zoom modifier.
fn shortcut(desktop: &mut Desktop, key: &str, code: &str) {
  let macos = cfg!(target_os = "macos");
  desktop
    .tree
    .key_down_with_meta(key.into(), code.into(), false, !macos, false, macos);
  desktop.pass();
}

/// A wheel notch away from the user over the middle of the canvas.
fn wheel(desktop: &mut Desktop, delta_y: f32) {
  let bounds = desktop
    .tree
    .get_element_by_id_mut("pipeline-canvas-area")
    .and_then(|element| element.bounds())
    .expect("the canvas is laid out");
  let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::Start);
  desktop.tree.scroll(x, y, 0.0, delta_y, ScrollPhase::Scroll);
  desktop.tree.scroll(x, y, 0.0, 0.0, ScrollPhase::End);
  desktop.pass();
}

#[test]
fn canvas_zoom_controls_step_and_stop_at_the_ends() {
  let mut desktop = starter();
  assert_eq!(level(&mut desktop), "100%");
  desktop.press("pipeline-zoom-in");
  assert_eq!(level(&mut desktop), "110%");
  desktop.press("pipeline-zoom-out");
  desktop.press("pipeline-zoom-out");
  assert_eq!(level(&mut desktop), "90%");
  for _ in 0..12 {
    desktop.press("pipeline-zoom-out");
  }
  assert_eq!(level(&mut desktop), "25%");
  desktop.press(LEVEL);
  assert_eq!(level(&mut desktop), "100%");
  for _ in 0..12 {
    desktop.press("pipeline-zoom-in");
  }
  assert_eq!(level(&mut desktop), "200%");
}

#[test]
fn canvas_zoom_shortcuts_work_unless_a_text_field_has_the_focus() {
  let mut desktop = starter();
  shortcut(&mut desktop, "=", "Equal");
  assert_eq!(level(&mut desktop), "110%");
  shortcut(&mut desktop, "+", "Equal");
  assert_eq!(level(&mut desktop), "125%");
  shortcut(&mut desktop, "-", "Minus");
  assert_eq!(level(&mut desktop), "110%");
  shortcut(&mut desktop, "0", "Digit0");
  assert_eq!(level(&mut desktop), "100%");
  // Without the modifier the keys do nothing to the canvas.
  desktop.key("-", "Minus");
  assert_eq!(level(&mut desktop), "100%");
  // A focused text field keeps them.
  desktop.tab_to(NAME, MAX_STOPS);
  shortcut(&mut desktop, "=", "Equal");
  assert_eq!(level(&mut desktop), "100%");
}

#[test]
fn canvas_wheel_zooms_with_the_modifier_and_pans_without_it() {
  let mut desktop = starter();
  modifier(&mut desktop.tree, true);
  wheel(&mut desktop, 40.0);
  assert_eq!(level(&mut desktop), "110%");
  wheel(&mut desktop, -40.0);
  assert_eq!(level(&mut desktop), "100%");
  modifier(&mut desktop.tree, false);
  wheel(&mut desktop, 40.0);
  assert_eq!(level(&mut desktop), "100%");
}

#[test]
fn canvas_fit_shows_the_whole_pipeline_smaller_in_a_smaller_window() {
  let mut desktop = starter();
  let percent = |desktop: &mut Desktop| -> u32 {
    let shown = level(desktop);
    shown.trim_end_matches('%').parse().expect("a percentage")
  };
  desktop.press("pipeline-zoom-fit");
  let fitted = percent(&mut desktop);
  // The starter is a little taller than the canvas under its problems strip; fit never enlarges.
  assert!((50..100).contains(&fitted), "{fitted}%");
  desktop.tree.resize(1100, 700);
  desktop.pass();
  desktop.press("pipeline-zoom-fit");
  let smaller = percent(&mut desktop);
  assert!((25..fitted).contains(&smaller), "{smaller}% after {fitted}%");
  desktop.press(LEVEL);
  assert_eq!(level(&mut desktop), "100%");
}

#[test]
fn canvas_zoom_is_kept_for_the_pipeline_when_it_is_opened_again() {
  let mut desktop = starter();
  desktop.press("pipeline-zoom-out");
  assert_eq!(level(&mut desktop), "90%");
  desktop.press("pipeline-breadcrumb-library");
  desktop.wait_for("pipeline-row");
  desktop.press("pipeline-row");
  desktop.wait_for(LEVEL);
  assert_eq!(level(&mut desktop), "90%");
}

fn name(value: &str) -> Name {
  Name::new(value).expect("valid name")
}

fn key(value: &str) -> StepKey {
  StepKey::new(value).expect("valid key")
}

/// A published Builder profile and the unpublished pipeline Delivery, shaped like the design's
/// (`yf6XH`): Research, Plan, Implement, then Run checks beside Review, then Create pull request.
/// A failed check returns to Implement, which the route note under the graph says.
fn delivery(repo: &Path) {
  let mut db = ProjectDb::open(repo).expect("open the project database");
  let now = Timestamp::now();
  let profile = AgentProfile::new(
    name("Builder"),
    String::new(),
    PromptText::new("Build.").expect("valid prompt"),
    ConnectionId::generate(),
    ModelId::new("gpt-5.5").expect("valid model"),
    ProviderSettings::default(),
  );
  let builder = db
    .create_resource(Slug::new("builder").expect("slug"), name("Builder"), &profile, now)
    .expect("profile")
    .id();
  db.publish_draft::<AgentProfile>(builder, now)
    .expect("publish the profile");
  let step = |value: &str, title: &str, after: &[&str]| Step {
    key: key(value),
    name: name(title),
    kind: StepKind::Agent {
      profile: Some(builder),
      repository: RepositoryAccess::Write,
      model: None,
      permissions: StepPermissions::default(),
    },
    instructions: PromptText::new("").expect("valid prompt"),
    depends_on: after.iter().map(|dependency| key(dependency)).collect(),
    approval_gate: false,
    on_failure: None,
    budget: None,
  };
  let checks = Step {
    on_failure: Some(FailureRoute {
      return_to: key("implement"),
      max_retries: NonZeroU32::new(2).expect("above zero"),
    }),
    ..step("checks", "Run checks", &["implement"])
  };
  let steps = vec![
    step("research", "Research", &[]),
    step("plan", "Plan", &["research"]),
    step("implement", "Implement", &["plan"]),
    checks,
    step("review", "Review", &["implement"]),
    step("pr", "Create pull request", &["checks", "review"]),
  ];
  let pipeline = Pipeline::new(steps).expect("valid pipeline");
  db.create_resource(Slug::new("delivery").expect("slug"), name("Delivery"), &pipeline, now)
    .expect("pipeline");
}

/// Delivery published from its editor, at the design's window size: the published notice shows.
fn published() -> Desktop {
  let mut desktop = Desktop::with_seeded_project(delivery);
  desktop.wait_for("nav-pipelines");
  desktop.press("nav-pipelines");
  desktop.wait_for("pipeline-row");
  desktop.press("pipeline-row");
  desktop.wait_for(NAME);
  desktop.press("pipeline-publish");
  desktop.wait_for("publish-confirm");
  desktop.press("publish-confirm");
  desktop.wait_for("pipeline-published-notice");
  desktop
}

fn bounds(desktop: &mut Desktop, id: &str) -> ElementRect {
  desktop
    .tree
    .get_element_by_id_mut(id)
    .and_then(|element| element.bounds())
    .unwrap_or_else(|| panic!("{id} is laid out"))
}

fn resize(desktop: &mut Desktop, width: u32, height: u32) {
  desktop.tree.resize(width, height);
  desktop.pass();
}

/// Waits until the inspector shows the step named `title`, which a press on its card selected.
fn selects(desktop: &mut Desktop, title: &str) {
  desktop.settle(title, |tree| {
    tree
      .get_element_by_id_mut(NAME)
      .and_then(|element| element.as_text_input())
      .is_some_and(|input| input.value() == title)
  });
}

/// A verifier saw the published notice cover the end of the route notes in windows smaller than
/// the design, where neither panning nor Fit brought them out. The notice's part of the canvas is
/// left free when fitting and panning; at the design's size the canvas shows as before.
#[test]
fn canvas_published_notice_never_hides_the_end_of_the_graph() {
  let mut desktop = published();
  // At the design's size the pipeline shows at 100 % from its top left, as before.
  assert_eq!(level(&mut desktop), "100%");
  desktop.press("step-card-pr");
  selects(&mut desktop, "Create pull request");
  desktop.press("step-card-research");
  selects(&mut desktop, "Research");

  resize(&mut desktop, 1200, 800);
  desktop.press("pipeline-zoom-fit");
  let percent: f32 = level(&mut desktop).trim_end_matches('%').parse().expect("a percentage");
  let (area, graph, notice) = (
    bounds(&mut desktop, "pipeline-canvas-area"),
    bounds(&mut desktop, "pipeline-canvas-graph"),
    bounds(&mut desktop, "pipeline-published-notice"),
  );
  assert!(notice.y < area.y + area.height, "the notice covers the canvas");
  // The shown level is rounded: the fitted graph is at least half a percent smaller.
  let fitted_bottom = area.y + graph.height * (percent - 0.5) / 100.0;
  assert!(
    fitted_bottom <= notice.y,
    "Fit at {percent}% ends the graph at {fitted_bottom}, under the notice at {}",
    notice.y
  );

  // Panned to its end at 100 %, the graph ends above the notice: the last card is where it is
  // only when the notice's part of the canvas is left free.
  resize(&mut desktop, 1280, 720);
  desktop.press(LEVEL);
  wheel(&mut desktop, -4000.0);
  let (area, graph, notice, card) = (
    bounds(&mut desktop, "pipeline-canvas-area"),
    bounds(&mut desktop, "pipeline-canvas-graph"),
    bounds(&mut desktop, "pipeline-published-notice"),
    bounds(&mut desktop, "step-card-pr"),
  );
  let scrolled = graph.height - (notice.y - area.y);
  assert!(scrolled > 0.0, "the graph is taller than the free part of the canvas");
  let (x, y) = (card.x + card.width / 2.0, card.y - scrolled + 20.0);
  desktop.tree.mouse_down(x, y, MouseButton::Left);
  desktop.tree.mouse_up(x, y, MouseButton::Left);
  desktop.pass();
  selects(&mut desktop, "Create pull request");
}
