//! While the project's *Dangerous auto-approve mode* is on, the bar's right begins with its chip (`Danger chip` `D3OhA`
//! in `i0Aqq`), which opens the project's Settings › Runs; nothing shows while it is off.

use orchester_app::routes::ProjectSettingsTab;

use super::*;

#[test]
fn status_dangerous_chip_shows_while_on_and_opens_runs_settings() {
  let mut off = Bar::of(facts(Some(kontur(
    runner(RunnerState::Online),
    ProjectTasks::default(),
  ))));
  assert!(off.texts("status-dangerous").is_none(), "nothing while it is off");
  let tasks = ProjectTasks {
    dangerous_auto_approve: true,
    ..ProjectTasks::default()
  };
  let mut bar = Bar::of(facts(Some(kontur(runner(RunnerState::Online), tasks))));
  assert_eq!(
    bar.texts("status-dangerous"),
    Some(vec!["Dangerous auto-approve".to_owned()])
  );
  let (chip, health) = (bar.bounds("status-dangerous-chip"), bar.bounds("status-health"));
  assert!((chip.height - 18.0).abs() < 0.5, "18 high: {chip:?}");
  assert!(chip.x < health.x, "before the health dot");
  bar.press("status-dangerous");
  assert_eq!(
    bar.asked(),
    vec![BarAction::Go(
      Route::Project(project_id(), ProjectPage::Settings(ProjectSettingsTab::Runs)),
      OpenIn::ActiveTab
    )]
  );
}
