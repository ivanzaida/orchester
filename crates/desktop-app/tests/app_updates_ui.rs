//! Settings › Updates (`wSMNG`, the update row's states `c69Eti` of `Auto-updater · redesign` `acFkz`): the section as
//! its facts say, headless at the design's width, with what its presses ask for; the route the status bar's item and
//! the notification open; the notification of a ready update once per version; and in a window, the section before
//! *This computer*, the channel and the switch stored, and *Version* naming the channel.
//!
//! `cargo test -p orchester-app --test app_updates_ui app_updates_ui`

mod support;

use std::{path::PathBuf, sync::Arc};

use lurq::app::{App, Tree, events::MouseButton};
use orchester_api::releases::ReleaseVersion;
use orchester_app::{
  i18n,
  notifications::update_ready::{ReadyToTell, to_tell},
  routes::{Route, UpdatesShow},
  theme::{self, Mode},
  updates::{SectionProps, UpdateAction, UpdatesFacts, UpdatesSection},
};
use orchester_core::{
  app_update::{Channel, UpdateSettings},
  time::Timestamp,
};
use orchester_workspace::{
  app_updates::{
    CopyReason, NewestRelease, ReadyUpdate, UpdateFailure, UpdateState,
    install::{InstallFailure, InstallStep, JustUpdated},
  },
  display,
};
use parking_lot::Mutex;
use support::Window;

/// When the cloud last answered in these tests.
const CHECKED: i64 = 1_790_000_000_000;

fn version(text: &str) -> ReleaseVersion {
  ReleaseVersion::parse(text).expect("a version")
}

fn facts(state: UpdateState) -> UpdatesFacts {
  UpdatesFacts {
    running: "0.3.0".to_owned(),
    state,
    just_updated: None,
    settings: Some(UpdateSettings::default()),
    last_checked: Some(Some(Timestamp::from_unix_millis(CHECKED))),
    problem: None,
    show: None,
  }
}

fn ready(critical: bool) -> ReadyUpdate {
  ReadyUpdate {
    version: version(if critical { "0.4.1" } else { "0.4.0" }),
    notes: "- Review › Merges: every merge attempt and why it did not land.".to_owned(),
    critical,
    installer: PathBuf::from("C:/Orchester/updates/Orchester-0.4.0-setup.exe"),
  }
}

fn newest() -> NewestRelease {
  NewestRelease {
    version: version("0.4.0"),
    download: "https://api.orchester.cloud/v1/releases/0.4.0/files/Orchester-0.4.0-setup.exe".to_owned(),
  }
}

/// The section mounted alone at the design's 720 column, and what its presses asked for.
struct Section {
  app: App,
  tree: Tree,
  asked: Arc<Mutex<Vec<UpdateAction>>>,
}

impl Section {
  fn of(facts: UpdatesFacts) -> Self {
    let mut app = App::new();
    theme::install(&mut app, Mode::Dark);
    i18n::install(&app).expect("bundled locales load");
    let mut tree = Tree::new();
    tree.resize(720, 900);
    orchester_app::ui::skeleton::install(&mut tree);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let record = asked.clone();
    let dispatch = Arc::new(move |action| record.lock().push(action));
    tree.mount_root::<UpdatesSection>(&mut app, SectionProps { facts, dispatch });
    tree.pass_headless(&mut app);
    Self { app, tree, asked }
  }

  fn texts(&mut self, id: &str) -> Option<Vec<String>> {
    let element = self.tree.get_element_by_id(id)?;
    let mut all = Vec::new();
    collect(element, &mut all);
    all.retain(|text| !text.chars().all(|glyph| ('\u{e000}'..='\u{f8ff}').contains(&glyph)));
    Some(all)
  }

  fn text(&mut self, id: &str) -> String {
    self.texts(id).unwrap_or_else(|| panic!("{id} shows")).join(" ")
  }

  fn press(&mut self, id: &str) {
    let bounds = self
      .tree
      .get_element_by_id_mut(id)
      .and_then(|element| element.bounds())
      .unwrap_or_else(|| panic!("{id} is laid out"));
    let (x, y) = (bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
    self.tree.mouse_move(x, y);
    self.tree.mouse_down(x, y, MouseButton::Left);
    self.tree.mouse_up(x, y, MouseButton::Left);
    self.tree.pass_headless(&mut self.app);
  }

  fn asked(&self) -> Vec<UpdateAction> {
    self.asked.lock().clone()
  }
}

fn collect(element: lurq::node::element::ElementRef<'_>, into: &mut Vec<String>) {
  into.extend(element.text_content().map(str::to_owned));
  for child in element.children() {
    collect(child, into);
  }
}

/// The update row's label and hint, and its actions' words.
fn row(section: &mut Section) -> (String, String, Vec<String>) {
  (
    section.text("settings-update-label"),
    section.text("settings-update-hint"),
    section.texts("settings-update-actions").unwrap_or_default(),
  )
}

#[test]
fn app_updates_ui_up_to_date_says_the_version_channel_and_check_and_checks_now() {
  let mut section = Section::of(facts(UpdateState::UpToDate {
    checked_at: Timestamp::from_unix_millis(CHECKED),
  }));
  let time = display::clock(Timestamp::from_unix_millis(CHECKED));
  assert_eq!(
    row(&mut section),
    (
      "Orchester is up to date".to_owned(),
      format!("0.3.0 · Stable · checked {time}"),
      vec!["Check now".to_owned()]
    )
  );
  section.press("settings-update-check-now");
  assert_eq!(section.asked(), [UpdateAction::CheckNow]);
}

#[test]
fn app_updates_ui_checking_and_installing_offer_nothing() {
  let mut checking = Section::of(facts(UpdateState::Checking));
  assert_eq!(
    row(&mut checking),
    (
      "Checking for updates…".to_owned(),
      "Asking Orchester Cloud for the newest Stable release".to_owned(),
      Vec::new()
    )
  );
  let mut installing = Section::of(facts(UpdateState::Installing {
    update: ready(false),
    step: InstallStep::BackingUp,
  }));
  let (label, hint, _) = row(&mut installing);
  assert_eq!(label, "Installing Orchester 0.4.0");
  assert_eq!(hint, "Your data is copied to backups first");
}

#[test]
fn app_updates_ui_a_download_shows_its_share_and_pauses_and_resumes() {
  let mut downloading = Section::of(facts(UpdateState::Downloading {
    version: version("0.4.0"),
    received: 38_400_000,
    total: 91_000_000,
  }));
  assert_eq!(
    row(&mut downloading),
    (
      "Downloading Orchester 0.4.0 · 42%".to_owned(),
      "38 of 91 MB · it is checked against its signature once downloaded".to_owned(),
      vec!["Pause".to_owned()]
    )
  );
  assert!(
    downloading.tree.get_element_by_id("settings-update-progress").is_some(),
    "the bar under the row"
  );
  downloading.press("settings-update-pause");
  assert_eq!(downloading.asked(), [UpdateAction::Pause]);

  let mut paused = Section::of(facts(UpdateState::Paused {
    version: version("0.4.0"),
    received: 38_400_000,
    total: 91_000_000,
  }));
  assert_eq!(
    paused.text("settings-update-label"),
    "Download of Orchester 0.4.0 paused · 42%"
  );
  paused.press("settings-update-resume");
  assert_eq!(paused.asked(), [UpdateAction::Resume]);
}

#[test]
fn app_updates_ui_a_ready_update_restarts_and_opens_its_notes() {
  let mut section = Section::of(facts(UpdateState::Ready(ready(false))));
  assert_eq!(
    row(&mut section),
    (
      "Orchester 0.4.0 is ready".to_owned(),
      "Downloaded and checked · installs when no run is at work, or now".to_owned(),
      vec!["What's new".to_owned(), "Restart to update".to_owned()]
    )
  );
  assert_eq!(section.texts("settings-update-notes"), None, "closed until asked");
  section.press("settings-update-whats-new");
  let notes = section.text("settings-update-notes");
  assert!(notes.starts_with("What's new in 0.4.0"), "{notes}");
  assert!(notes.contains("every merge attempt"), "{notes}");
  section.press("settings-update-restart");
  assert_eq!(section.asked(), [UpdateAction::Restart]);
}

#[test]
fn app_updates_ui_waiting_for_runs_names_them_and_installs_now() {
  let runs = ["ORC-110", "ORC-346", "ORC-405"].map(str::to_owned).to_vec();
  let mut section = Section::of(facts(UpdateState::WaitingForRuns {
    update: ready(false),
    runs,
  }));
  let (label, hint, actions) = row(&mut section);
  assert_eq!(label, "Orchester 0.4.0 installs when 3 runs end");
  assert_eq!(hint, "ORC-110, ORC-346 and ORC-405 are at work");
  assert_eq!(actions, ["What's new", "Install now"]);
  section.press("settings-update-install-now");
  assert_eq!(section.asked(), [UpdateAction::Restart]);
}

#[test]
fn app_updates_ui_a_critical_update_says_so_and_restarts() {
  let mut section = Section::of(facts(UpdateState::Ready(ready(true))));
  let (label, hint, actions) = row(&mut section);
  assert_eq!(label, "Critical update 0.4.1");
  assert_eq!(
    hint,
    "It fixes a security problem and installs at the next restart; runs at work are waited for"
  );
  assert_eq!(actions, ["What's new", "Restart to update"]);
}

#[test]
fn app_updates_ui_a_failed_download_says_why_and_tries_again() {
  let mut section = Section::of(facts(UpdateState::Failed {
    version: version("0.4.0"),
    failure: UpdateFailure::BadSignature,
  }));
  assert_eq!(
    row(&mut section),
    (
      "Couldn't install 0.4.0: the download didn't match its signature".to_owned(),
      "The file was deleted; nothing was installed. Orchester tries again at the next check.".to_owned(),
      vec!["Try again".to_owned()]
    )
  );
  section.press("settings-update-try-again");
  assert_eq!(section.asked(), [UpdateAction::CheckNow]);
}

#[test]
fn app_updates_ui_an_install_that_failed_opens_the_installers_log() {
  let log = PathBuf::from("C:/Orchester/logs/update-0.4.0.log");
  let mut section = Section::of(facts(UpdateState::InstallFailed {
    version: version("0.4.0"),
    failure: InstallFailure {
      reason: "the installer ended before the app closed, with exit code 2".to_owned(),
      exit_code: Some(2),
      log: Some(log.clone()),
    },
  }));
  let (label, hint, _) = row(&mut section);
  assert_eq!(label, "Orchester 0.4.0 didn't install");
  assert!(hint.starts_with("the installer ended before the app closed"), "{hint}");
  section.press("settings-update-log");
  assert_eq!(section.asked(), [UpdateAction::OpenLog(log)]);
}

#[test]
fn app_updates_ui_an_unreachable_cloud_says_when_it_last_answered() {
  let mut section = Section::of(facts(UpdateState::CloudUnreachable { last_checked: None }));
  assert_eq!(
    row(&mut section),
    (
      "Couldn't reach Orchester Cloud".to_owned(),
      "It hasn't answered from here yet · Orchester tries again in 6 hours".to_owned(),
      vec!["Check now".to_owned()]
    )
  );
  let two_hours_ago = Timestamp::from_unix_millis(Timestamp::now().unix_millis() - 2 * 60 * 60 * 1000 - 1000);
  let mut section = Section::of(facts(UpdateState::CloudUnreachable {
    last_checked: Some(two_hours_ago),
  }));
  let hint = section.text("settings-update-hint");
  assert!(hint.starts_with("Last checked 2"), "{hint}");
  assert!(hint.ends_with("· Orchester tries again in 6 hours"), "{hint}");
}

#[test]
fn app_updates_ui_a_copy_that_does_not_update_itself_links_the_newest_release() {
  let mut section = Section::of(facts(UpdateState::NotThisCopy {
    reason: CopyReason::Qa,
    newest: Some(newest()),
  }));
  assert_eq!(
    row(&mut section),
    (
      "This copy doesn't update itself".to_owned(),
      "It's a QA copy, not the installer's copy. Orchester 0.4.0 is out.".to_owned(),
      vec!["Download 0.4.0".to_owned()]
    )
  );
  section.press("settings-update-download");
  assert_eq!(section.asked(), [UpdateAction::Open(newest().download)]);

  let mut alone = Section::of(facts(UpdateState::NotThisCopy {
    reason: CopyReason::Development,
    newest: None,
  }));
  assert_eq!(row(&mut alone).1, "It's a development build, not the installer's copy.");
  assert_eq!(alone.texts("settings-update-download"), None);
}

#[test]
fn app_updates_ui_channel_switch_and_last_check_are_rows_below() {
  let mut section = Section::of(facts(UpdateState::Checking));
  assert_eq!(section.text("settings-updates-title"), "Updates");
  assert_eq!(section.text("settings-update-channel-label"), "Channel");
  assert_eq!(section.text("settings-update-automatic-label"), "Update automatically");
  let time = display::clock(Timestamp::from_unix_millis(CHECKED));
  assert_eq!(
    section.text("settings-update-checked-hint"),
    format!("Orchester Cloud · {time}")
  );
  section.press("settings-update-channel-beta");
  section.press("settings-update-automatic");
  section.press("settings-update-checked-check-now");
  assert_eq!(
    section.asked(),
    [
      UpdateAction::Channel(Channel::Beta),
      UpdateAction::Automatic(false),
      UpdateAction::CheckNow
    ]
  );
}

#[test]
fn app_updates_ui_after_an_update_whats_new_opens_the_installed_notes() {
  let mut updated = facts(UpdateState::UpToDate {
    checked_at: Timestamp::from_unix_millis(CHECKED),
  });
  updated.just_updated = Some(JustUpdated {
    version: version("0.4.0"),
    notes: "Runs start faster.".to_owned(),
  });
  updated.show = Some(UpdatesShow::WhatsNew);
  let mut section = Section::of(updated);
  let notes = section.text("settings-update-notes");
  assert!(
    notes.starts_with("What's new in 0.4.0"),
    "open as the route asks: {notes}"
  );
  assert!(notes.contains("Runs start faster."), "{notes}");
}

#[test]
fn app_updates_ui_the_route_names_the_section_and_whats_new() {
  for show in [UpdatesShow::Section, UpdatesShow::WhatsNew] {
    let route = Route::Updates(show);
    assert_eq!(Route::parse(&route.path()), Ok(route.clone()), "{}", route.path());
  }
  assert_eq!(
    Route::Updates(UpdatesShow::Section).path(),
    "/settings?tab=general&show=updates"
  );
  assert!(Route::parse("/settings?tab=general&show=elsewhere").is_err());
}

#[test]
fn app_updates_ui_a_ready_update_is_told_once_per_version() {
  let state = UpdateState::Ready(ready(false));
  assert_eq!(
    to_tell(&state, None),
    Some(ReadyToTell {
      version: "0.4.0".to_owned(),
      critical: false
    })
  );
  assert_eq!(to_tell(&state, Some("0.4.0")), None, "told already");
  assert!(to_tell(&state, Some("0.3.9")).is_some(), "a newer version is told");
  let waiting = UpdateState::WaitingForRuns {
    update: ready(true),
    runs: vec!["ORC-1".to_owned()],
  };
  assert_eq!(to_tell(&waiting, None).map(|told| told.critical), Some(true));
  assert_eq!(to_tell(&UpdateState::Checking, None), None);
}

/// Settings › General shows *Updates* before *This computer*, keeps the channel and the switch, and *Version* names
/// the channel.
#[test]
fn app_updates_ui_settings_general_stores_the_channel_and_the_switch() {
  let mut window = Window::open();
  window.navigation().go(Route::Updates(UpdatesShow::Section));
  window.until("the update settings", |window| {
    window.texts("settings-update-channel-stable").is_some()
  });
  let texts = window.texts("settings-general").expect("the tab shows");
  let at = |words: &str| texts.iter().position(|text| text == words);
  assert!(
    at("Updates") < at("This computer") && at("Updates").is_some(),
    "{texts:?}"
  );
  let updates = window.launch.services().expect("services").updates.clone();
  assert_eq!(updates.settings().expect("read"), UpdateSettings::default());
  window.until("the version to name the channel", |window| {
    window.text("settings-version-value").ends_with("· Stable")
  });

  window.press("settings-update-channel-beta");
  window.until("the channel to be stored", |_| {
    updates.settings().expect("read").channel == Channel::Beta
  });
  window.until("the version to follow it", |window| {
    window.text("settings-version-value").ends_with("· Beta")
  });
  window.press("settings-update-automatic");
  window.until("the switch to be stored", |_| {
    !updates.settings().expect("read").automatic
  });
  window.until("the saves to settle", |window| {
    window.text("settings-save-state") == "All changes saved"
  });
}
