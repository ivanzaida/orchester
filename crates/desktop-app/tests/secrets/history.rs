//! Secrets' tab, environment filter, chosen secret and panels are places of the tab's history: Back returns
//! to the one before and Forward comes back; a cancelled panel leaves no entry.
use orchester_app::routes::{ProjectPage, Route, SecretsPanel, SecretsTab, SecretsView};
use orchester_core::secret::Environment;

use super::{harness::Secrets, support::project_id};

fn shown(s: &Secrets) -> SecretsView {
  match s.window.navigation().route_now() {
    Route::Project(_, ProjectPage::Secrets(view)) => view,
    other => panic!("Secrets is not shown: {other:?}"),
  }
}

fn back(s: &mut Secrets) {
  s.window.navigation().back();
  s.window.pass();
}

fn forward(s: &mut Secrets) {
  s.window.navigation().forward();
  s.window.pass();
}

#[test]
fn secrets_choices_are_history_that_back_and_forward_walk() {
  let mut s = Secrets::new();
  let alpha = s.seed("ALPHA_TOKEN", Environment::Development);
  let beta = s.seed("BETA_TOKEN", Environment::Staging);
  s.list();
  let beta_row = format!("secret-row-{}", beta.id());
  s.until("both rows", |s| s.exists(&beta_row));
  s.click(&beta_row);
  assert_eq!(shown(&s).secret, Some(beta.id()));
  s.choose("secrets-environment", "Staging");
  assert_eq!(shown(&s).environment, Some(Environment::Staging));
  s.click("secrets-tab-usage");
  assert_eq!(shown(&s).tab, SecretsTab::Usage);
  assert!(s.exists("secrets-usage"));
  back(&mut s);
  assert_eq!(shown(&s).tab, SecretsTab::List);
  s.until("the filtered list", |s| s.exists(&beta_row));
  assert!(!s.exists(&format!("secret-row-{}", alpha.id())), "the filter is back");
  back(&mut s);
  assert_eq!(shown(&s).environment, None);
  s.until("every environment again", |s| {
    s.exists(&format!("secret-row-{}", alpha.id()))
  });
  back(&mut s);
  assert_eq!(shown(&s), SecretsView::default(), "the list as it opened");
  forward(&mut s);
  assert_eq!(shown(&s).secret, Some(beta.id()));
  forward(&mut s);
  forward(&mut s);
  assert_eq!(shown(&s).tab, SecretsTab::Usage);
  assert_eq!(
    s.window.navigation().route_now(),
    Route::Project(
      project_id(),
      ProjectPage::Secrets(SecretsView {
        tab: SecretsTab::Usage,
        environment: Some(Environment::Staging),
        secret: Some(beta.id()),
        panel: SecretsPanel::Inspect,
      })
    )
  );
}

#[test]
fn secrets_a_cancelled_panel_leaves_no_history_entry() {
  let mut s = Secrets::new();
  s.seed("ALPHA_TOKEN", Environment::Development);
  s.list();
  s.until("the inspector", |s| s.exists("secret-replace"));
  s.click("secrets-add");
  assert_eq!(shown(&s).panel, SecretsPanel::Add);
  s.click("secret-cancel");
  assert_eq!(shown(&s).panel, SecretsPanel::Inspect);
  assert!(
    !s.window.navigation().tabs_now().active().can_forward(),
    "Forward does not reopen the cancelled form"
  );
  s.click("secret-replace");
  assert_eq!(shown(&s).panel, SecretsPanel::Replace);
  back(&mut s);
  assert_eq!(
    shown(&s).panel,
    SecretsPanel::Inspect,
    "Back closes the panel it opened"
  );
  forward(&mut s);
  assert_eq!(shown(&s).panel, SecretsPanel::Replace);
}
