//! 06 a–b: model approvals remain quiet, open their recorded verdict, and collapse again.
use super::{click, laid_out, opened_in, text};

#[test]
fn card_model_approved_line_opens_recorded_verdict_and_five_proportional_risks() {
  for (title, probability) in [
    ("Check the app with jev", "0.97"),
    ("Review an approved tool request", "0.86"),
  ] {
    let (mut window, card, (project, key)) = opened_in("cards-auto-mode", title, "cards-approval-");
    let line = format!("{card}-line");
    assert_eq!(text(&mut window, &format!("{line}-lead")), "Approved by jev");
    assert!(text(&mut window, &format!("{line}-detail")).starts_with(&format!("· {probability} · ")));
    assert!(window.tree.get_element_by_id(&format!("{card}-details")).is_none());
    assert!(window.tree.get_element_by_id(&format!("{card}-approve")).is_none());
    click(&mut window, &line);
    assert_eq!(text(&mut window, &format!("{card}-model")), "jev · jev-1.14.0 · 183 ms");
    assert_eq!(
      text(&mut window, &format!("{card}-verdict")),
      format!("approve {probability} · threshold 0.80")
    );
    for (index, expected) in ["0.02", "0.10", "0.00", "0.11", "0.30"].into_iter().enumerate() {
      assert_eq!(text(&mut window, &format!("{card}-risk-{index}-value")), expected);
      let bar = laid_out(&mut window, &format!("{card}-risk-{index}-bar"));
      let probability: f32 = expected.parse().expect("probability");
      assert!((bar.width - 160.0 * probability).abs() < 1.0, "{bar:?}");
    }
    click(&mut window, &line);
    assert!(window.tree.get_element_by_id(&format!("{card}-details")).is_none());
    click(&mut window, &line);
    assert!(window.tree.get_element_by_id(&format!("{card}-details")).is_some());
    window.navigation().go(orchester_app::routes::Route::Project(
      project,
      orchester_app::routes::ProjectPage::Task(key, orchester_app::routes::TaskView::Summary),
    ));
    window.until("run details", |window| {
      window.tree.get_element_by_id("task-facts-more").is_some()
    });
    click(&mut window, "task-facts-more");
    window.until("reviewer count", |window| {
      window.tree.get_element_by_id("task-fact-approvals-value").is_some()
    });
    assert_eq!(text(&mut window, "task-fact-approvals-value"), "1 by the reviewer");
  }
}
