//! Secrets' environment filter, opened secret and panels are places of the tab's history: Back returns to the one
//! before and Forward comes back; a cancelled panel leaves no entry.
use orchester_app::routes::{ProjectPage, Route, SecretsPanel, SecretsView};
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

/// The environment filter shows only that environment's group, and is history like the secret opened from it.
#[test]
fn secrets_environment_filter_and_opened_secret_are_history() {
  let mut s = Secrets::new();
  let alpha = s.seed("ALPHA_TOKEN", Environment::Development);
  let beta = s.seed("BETA_TOKEN", Environment::Staging);
  s.list();
  let (alpha_row, beta_row) = (
    format!("secret-row-{}-row", alpha.id()),
    format!("secret-row-{}-row", beta.id()),
  );
  s.until("both rows", |s| s.exists(&alpha_row) && s.exists(&beta_row));
  s.click("secrets-environment-staging");
  assert_eq!(shown(&s).environment, Some(Environment::Staging));
  s.until("the filtered list", |s| !s.exists(&alpha_row));
  assert!(s.exists(&beta_row), "staging's secret stays");
  assert!(s.exists("secrets-group-staging"));
  assert!(
    !s.exists("secrets-group-development"),
    "only the chosen environment's group"
  );
  s.click(&beta_row);
  assert_eq!(shown(&s).secret, Some(beta.id()));
  s.until("the secret's page", |s| s.exists("secret-crumb-back"));
  back(&mut s);
  assert_eq!(shown(&s).secret, None);
  s.until("the filtered list again", |s| s.exists(&beta_row));
  assert!(!s.exists(&alpha_row), "the filter is back");
  back(&mut s);
  assert_eq!(shown(&s), SecretsView::default(), "the list as it opened");
  s.until("every environment again", |s| s.exists(&alpha_row));
  forward(&mut s);
  forward(&mut s);
  assert_eq!(
    s.window.navigation().route_now(),
    Route::Project(
      project_id(),
      ProjectPage::Secrets(SecretsView {
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
  let alpha = s.seed("ALPHA_TOKEN", Environment::Development);
  s.list();
  s.until("the list", |s| s.exists("secrets-add"));
  s.click("secrets-add");
  assert_eq!(shown(&s).panel, SecretsPanel::Add);
  s.until("the form", |s| s.exists("secret-cancel"));
  s.click("secret-cancel");
  assert_eq!(shown(&s).panel, SecretsPanel::Inspect);
  assert!(
    !s.window.navigation().tabs_now().active().can_forward(),
    "Forward does not reopen the cancelled form"
  );
  let row = format!("secret-row-{}-row", alpha.id());
  s.until("the list", |s| s.exists(&row));
  s.click(&row);
  s.until("the secret's page", |s| s.exists("secret-replace"));
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
