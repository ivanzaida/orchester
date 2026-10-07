//! The Usage dashboard (`C2pTYF`, `p4eVV6`): the sidebar item and the status bar's *Tokens today*
//! open it; a finished run shows in the summary, the chart, its table and *Most expensive*; a
//! project without runs says so and offers the last seven days; plan limits without reports say
//! so; the warning threshold saves for every project; All projects shows every project's usage;
//! axis labels stay whole inside their charts and narrow tables keep readable titles.
//! `cargo test -p orchester-desktop --test form_keyboard usage`.

use lurq::canvas::CanvasItemShape;

use crate::{
  github::{contains_text, shows},
  harness::Desktop,
  task_run::{create_and_run, fill_new_task, init_repository, latest_run, programs, seed, set_text},
};

fn has(desktop: &mut Desktop, id: &str) -> bool {
  desktop.tree.get_element_by_id(id).is_some()
}

fn shows_text(desktop: &mut Desktop, id: &str, text: &str) -> bool {
  desktop
    .tree
    .get_element_by_id(id)
    .is_some_and(|element| contains_text(element, text))
}

#[test]
fn usage_dashboard_shows_a_finished_run_from_the_sidebar_and_the_status_bar() {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  create_and_run(&mut desktop, "Append one line to README.md.", "1400000");
  desktop.wait_until("the run to finish", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().is_terminal())
  });
  desktop.press("nav-usage");
  desktop.wait_until("the run in the summary", |desktop| {
    shows_text(desktop, "usage-summary-runs", "1 finished")
  });
  assert!(shows(&mut desktop, "usage-readback", "compared with"));
  desktop.wait_for("usage-tokens-chart");
  assert!(
    shows(&mut desktop, "usage-expensive", "KON-1"),
    "the task is the most expensive"
  );
  desktop.press("usage-tokens-view-1");
  desktop.wait_for("usage-tokens-table");
  // Below the fold: clicked where it is, as the page would be scrolled to it.
  desktop.click("usage-expensive-mode-1");
  desktop.wait_for("usage-expensive-runs");
  assert!(shows(&mut desktop, "usage-expensive-runs", "KON-1 · Run 1"));

  desktop.press("nav-overview");
  desktop.wait_for("status-tokens");
  desktop.press("status-tokens");
  desktop.wait_for("usage-summary");
}

#[test]
fn usage_without_runs_says_so_offers_seven_days_and_no_plan_limits() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-usage");
  desktop.press("nav-usage");
  desktop.wait_until("the empty week", |desktop| {
    shows_text(desktop, "usage-empty", "No runs in Kontur in this period")
  });
  assert!(shows(&mut desktop, "usage-limits", "No plan limits recorded yet"));
  assert!(
    shows(&mut desktop, "usage-summary-cache", "—"),
    "a share without input is a dash"
  );
  desktop.press("usage-period-0");
  desktop.wait_until("the empty day", |desktop| {
    shows_text(desktop, "usage-empty", "No runs in Kontur today")
  });
  desktop.press("usage-show-week");
  desktop.wait_until("the week again", |desktop| {
    shows_text(desktop, "usage-empty", "in this period") && !has(desktop, "usage-show-week")
  });
}

fn set_threshold(desktop: &mut Desktop, text: &str) {
  desktop
    .tree
    .get_element_by_id_mut("usage-threshold")
    .and_then(|element| element.as_text_input())
    .expect("the threshold field")
    .set_value(text);
  desktop.pass();
}

#[test]
fn usage_warning_threshold_saves_for_every_project() {
  let mut desktop = Desktop::with_project();
  desktop.wait_for("nav-usage");
  desktop.press("nav-usage");
  desktop.wait_for("usage-limits-menu");
  desktop.press("usage-limits-menu");
  desktop.wait_for("usage-threshold");
  set_threshold(&mut desktop, "101");
  assert!(shows(
    &mut desktop,
    "usage-threshold-popover",
    "Enter a whole number from 1 to 100."
  ));
  set_threshold(&mut desktop, "70");
  desktop.wait_until("the threshold saved", |desktop| {
    shows_text(desktop, "usage-threshold-popover", "Saved for all projects")
  });
}

#[test]
fn usage_tab_of_all_projects_reads_every_project() {
  let mut desktop = Desktop::with_other_project(|_| {}, |_| {});
  desktop.wait_for("project-selector");
  desktop.press("nav-all-projects");
  desktop.wait_for("all-projects-tab-usage");
  desktop.press("all-projects-tab-usage");
  desktop.wait_until("both projects read", |desktop| {
    shows_text(desktop, "usage-readback", "2 projects")
  });
  assert!(
    has(&mut desktop, "usage-filter-project"),
    "Project comes first among the filters"
  );
  desktop.press("all-projects-tab-projects");
  desktop.wait_for("project-row-1");
}

/// A task title too long for the Task column of a narrow window.
const LONG_TITLE: &str = "Migrate the settings to the project database";

/// Runs a task named [`LONG_TITLE`] to its end and opens Usage.
fn usage_after_a_long_titled_run() -> Desktop {
  let (runner, fake) = programs();
  let mut desktop = Desktop::with_runner(runner, init_repository, |repo| seed(repo, &fake, false));
  fill_new_task(&mut desktop, "Append one line to README.md.", "1400000");
  set_text(&mut desktop, "task-title", LONG_TITLE);
  desktop.wait_until("the task page", |desktop| {
    // Pressing before the runner is online fails visibly; pressing again retries the same run.
    if !has(desktop, "task-page") && has(desktop, "create-and-run") {
      desktop.click("create-and-run");
    }
    has(desktop, "task-page")
  });
  desktop.wait_until("the run to finish", |desktop| {
    latest_run(desktop.repo()).is_some_and(|run| run.status().is_terminal())
  });
  desktop.press("nav-usage");
  desktop.wait_until("the run in the summary", |desktop| {
    shows_text(desktop, "usage-summary-runs", "1 finished")
  });
  desktop
}

/// An x axis label a chart drew: its left edge, width and text.
#[derive(Debug)]
struct AxisLabel {
  left: f32,
  width: f32,
  text: String,
}

/// The canvas's width and the x axis labels it drew, left to right.
fn axis_labels(desktop: &mut Desktop, id: &str) -> Option<(f32, Vec<AxisLabel>)> {
  let canvas = desktop.tree.get_element_by_id(id)?.as_canvas()?;
  let mut labels: Vec<AxisLabel> = canvas
    .items()
    .iter()
    .filter(|item| item.role == "label")
    .filter_map(|item| match item.shape {
      CanvasItemShape::Rect { x, width, .. } => Some(AxisLabel {
        left: x,
        width,
        text: item.label.clone()?,
      }),
      _ => None,
    })
    .collect();
  labels.sort_by(|left, right| left.left.total_cmp(&right.left));
  Some((canvas.size().width, labels))
}

/// Every label lies whole inside the canvas and none runs into the next; the last reads `last`.
fn assert_labels_inside(desktop: &mut Desktop, id: &str, last: &str) {
  desktop.wait_until("the chart's axis labels", |desktop| {
    axis_labels(desktop, id).is_some_and(|(_, labels)| labels.len() > 1)
  });
  let (width, labels) = axis_labels(desktop, id).expect("the chart's canvas");
  for label in &labels {
    let right = label.left + label.width;
    assert!(
      label.left >= 0.0 && right <= width + 0.5,
      "{id}: {label:?} ends at {right}, outside 0..{width}"
    );
  }
  for pair in labels.windows(2) {
    assert!(pair[0].left + pair[0].width <= pair[1].left, "{id}: {pair:?} overlap");
  }
  assert_eq!(labels.last().map(|label| label.text.as_str()), Some(last), "{id}");
}

#[test]
fn usage_axis_labels_stay_inside_the_plot_and_narrow_titles_give_way_with_room() {
  let mut desktop = usage_after_a_long_titled_run();
  desktop.press("usage-period-2");
  desktop.wait_until("30 days", |desktop| {
    shows_text(desktop, "usage-readback", "compared with")
  });
  // 30 day slots are narrower than "Today": it ends at the plot's right end instead of past it.
  assert_labels_inside(&mut desktop, "usage-tokens-chart", "Today");
  assert_labels_inside(&mut desktop, "usage-cache-chart", "Today");

  desktop.tree.resize(960, 960);
  desktop.pass();
  desktop.wait_for("usage-expensive-title-0");
  let title = desktop.bounds("usage-expensive-title-0");
  let pipeline = desktop.bounds("usage-expensive-tasks-heading-1");
  let runs = desktop.bounds("usage-expensive-tasks-heading-2");
  assert!(
    title.x + title.width <= pipeline.x,
    "the title ends in its own column: {title:?} before {pipeline:?}"
  );
  assert!(title.width >= 120.0, "the title keeps readable room: {title:?}");
  assert!(
    runs.x - pipeline.x >= 80.0,
    "the pipeline keeps readable room: {pipeline:?} to {runs:?}"
  );
}
