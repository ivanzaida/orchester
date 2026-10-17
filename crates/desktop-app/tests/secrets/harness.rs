//! A worktree-local project and an in-memory vault. Tests never touch the OS keychain.
use std::{
  collections::HashMap,
  fs,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
  },
};

use lurq::app::{App, Tree};
use orchester_app::{
  i18n,
  mcp::{McpKind, McpLink, McpSettings, McpStatus, WindowLink},
  root::{Root, RootProps, Start},
  routes::{ProjectPage, Route},
  theme::{self, Mode},
};
use orchester_core::{
  secret::{Environment, NewSecret, Secret, SecretKind, SecretName, SecretValue, SecretVault, VaultError, VaultKey},
  store::SecretStore,
};
use orchester_store::ProjectDb;
use orchester_workspace::{
  connections::mcp::{McpServerText, save_in, secret_reference},
  secrets::{Vault, data},
  tasks::outside::Outside,
};
use parking_lot::{Condvar, Mutex};
use tempfile::TempDir;
use tokio::runtime::Runtime;

use super::support::{Data, Window, project_id};

/// Synthetic test credentials; never asserted with value-bearing diagnostics.
pub const VALUE: &str = "fixture-only-credential";
pub const NEXT: &str = "fixture-only-replacement";
#[derive(Default)]
pub struct MemoryVault {
  values: Mutex<HashMap<VaultKey, SecretValue>>,
  pub fail: AtomicBool,
  pub reads: AtomicUsize,
  pub stores: AtomicUsize,
  blocked: Mutex<bool>,
  released: Condvar,
}
impl MemoryVault {
  pub fn block(&self) {
    *self.blocked.lock() = true;
  }
  pub fn release(&self) {
    *self.blocked.lock() = false;
    self.released.notify_all();
  }
  fn ready(&self) -> Result<(), VaultError> {
    let mut blocked = self.blocked.lock();
    while *blocked {
      self.released.wait(&mut blocked);
    }
    if self.fail.load(Ordering::SeqCst) {
      Err(VaultError::AccessDenied("fixture denial".into()))
    } else {
      Ok(())
    }
  }
  pub fn contains(&self, key: &VaultKey, expected: &str) -> bool {
    self
      .values
      .lock()
      .get(key)
      .is_some_and(|value| value.expose() == expected)
  }
}
impl SecretVault for MemoryVault {
  fn store(&self, key: &VaultKey, value: &SecretValue) -> Result<(), VaultError> {
    self.stores.fetch_add(1, Ordering::SeqCst);
    self.ready()?;
    self.values.lock().insert(
      key.clone(),
      SecretValue::new(value.expose().to_owned()).expect("nonblank fixture"),
    );
    Ok(())
  }
  fn read(&self, key: &VaultKey) -> Result<SecretValue, VaultError> {
    self.reads.fetch_add(1, Ordering::SeqCst);
    self
      .values
      .lock()
      .get(key)
      .map(|value| SecretValue::new(value.expose().to_owned()).expect("fixture"))
      .ok_or(VaultError::NotFound)
  }
  fn delete(&self, key: &VaultKey) -> Result<(), VaultError> {
    self.ready()?;
    self.values.lock().remove(key);
    Ok(())
  }
}

pub struct Secrets {
  pub window: Window,
  pub repo: PathBuf,
  pub vault: Arc<MemoryVault>,
}
impl Drop for Secrets {
  fn drop(&mut self) {
    self.vault.release();
  }
}
impl Secrets {
  pub fn new() -> Self {
    let scratch = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.tmp/secrets-tests");
    fs::create_dir_all(&scratch).expect("test scratch");
    let data = Data::at(
      TempDir::new_in(scratch).expect("test data"),
      Runtime::new().expect("runtime"),
    );
    let launch = data.launch();
    let open = data.add_kontur(launch.workspace().expect("session"));
    let vault = Arc::new(MemoryVault::default());
    let mut start = launch.start();
    let Start::Ready(ready) = &mut start else {
      panic!("ready session")
    };
    ready.services.vault = Vault::with(vault.clone());
    let mut app = App::new();
    app.set_tokio_handle(data.runtime.handle().clone());
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("locales");
    let mut tree = Tree::new();
    tree.resize(1440, 900);
    let link = WindowLink::default();
    tree.mount_root::<Root>(
      &mut app,
      RootProps {
        link: link.clone(),
        mcp: McpLink::without_server(McpStatus::unavailable(McpKind::Installation, McpSettings::of_build())),
        start,
      },
    );
    tree.pass_headless(&mut app);
    Self {
      window: Window {
        app,
        tree,
        link,
        launch,
        data,
      },
      repo: open.repo_root,
      vault,
    }
  }
  pub fn list(&mut self) {
    self
      .window
      .navigation()
      .go(Route::Project(project_id(), ProjectPage::Secrets(Default::default())));
    self.until("secrets list", |s| s.exists("secrets-body"));
  }
  pub fn seed(&self, name: &str, environment: Environment) -> Secret {
    let id = data::add_in(
      &self.repo,
      self.vault.as_ref(),
      NewSecret {
        name: SecretName::new(name).expect("name"),
        kind: SecretKind::ApiKey,
        environment,
      },
      &SecretValue::new(VALUE.into()).expect("fixture"),
    )
    .expect("seed secret");
    self.db().secret(id).expect("read").expect("secret")
  }
  pub fn bind(&self, secret: &Secret) {
    let text = McpServerText {
      name: "kontur-docs".into(),
      program: std::env::current_exe()
        .expect("test executable")
        .to_string_lossy()
        .into_owned(),
      variables: format!("TOKEN={}", secret_reference(secret)),
      ..McpServerText::new_server()
    };
    save_in(
      &self.repo,
      None,
      text.draft(&[], std::slice::from_ref(secret)).expect("draft"),
    )
    .expect("server");
  }
  pub fn db(&self) -> ProjectDb {
    ProjectDb::open(&self.repo).expect("project database")
  }
  pub fn refresh(&mut self) {
    let changes = self.window.launch.start();
    if let Start::Ready(ready) = changes {
      ready.services.changes.record(project_id(), Outside::Connections);
    }
    self.window.pass();
  }
  pub fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
      self.window.tree.tick_futures();
      self.window.pass();
      if done(self) {
        return;
      }
      assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
      std::thread::sleep(std::time::Duration::from_millis(5));
    }
  }
  pub fn exists(&mut self, id: &str) -> bool {
    self.window.tree.get_element_by_id(id).is_some()
  }
  pub fn click(&mut self, id: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .unwrap_or_else(|| panic!("{id} exists"))
      .click();
    self.window.pass();
  }
  pub fn set(&mut self, id: &str, value: &str) {
    self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_text_input())
      .unwrap_or_else(|| panic!("{id} input"))
      .set_value(value);
    self.window.pass();
  }
  pub fn choose(&mut self, id: &str, label: &str) {
    let mut select = self
      .window
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.as_select())
      .expect("select");
    let index = select
      .labels()
      .iter()
      .position(|value| value.as_ref() == label)
      .expect("option");
    select.commit(index);
    self.window.pass();
  }
  pub fn text(&mut self, id: &str) -> String {
    self.window.texts(id).unwrap_or_else(|| panic!("{id} exists")).join(" ")
  }
  pub fn private(&mut self) {
    let input = self
      .window
      .tree
      .get_element_by_id_mut("secret-value")
      .and_then(|element| element.as_text_input())
      .expect("credential input");
    assert!(input.is_masked());
    // The native input buffer is available to ElementRef; privacy.rs checks the actual masked MCP tree.
    assert!(!self.text("secret-form-notice").contains(VALUE));
    assert!(!self.text("secret-form-notice").contains(NEXT));
  }
}
