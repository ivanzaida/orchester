//! The desktop app's own updates (`docs/cloud-api.md`, "App releases"): Settings › Updates in the application
//! database ([`UpdateSettings`]), the check at the start, every [`CHECK_EVERY`] and on *Check now* — Orchester Cloud's
//! anonymous update check with this copy's version, platform, architecture, channel and install id, no sign-in — and
//! the download of an offered installer into the data folder's [`UPDATES_FOLDER`], resumed after an interruption and
//! kept only once its SHA-256 and its signature by a release key built into the app check out ([`download`]). Only a
//! copy the installer installed on Windows downloads ([`ThisCopy`]); every other copy still asks, to name the newest
//! release and its download. What the window and the app's MCP server show is one [`UpdateState`]; nothing here
//! installs an update.

mod copy;
mod download;
mod state;

use std::{
  env,
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_api::{
  client::{ClientError, CloudClient},
  releases::{
    AcceptedReleaseKeys, OfferedFile, OfferedRelease, ReleaseArch, ReleasePlatform, ReleaseVersion, UpdateCheck,
  },
};
use orchester_core::{
  app_update::{Channel, UpdateSettings},
  time::Timestamp,
};
use orchester_store::StoreError;
use parking_lot::Mutex;
use tokio::sync::watch;

use self::download::{Download, Fetched};
pub use self::{
  copy::{CopyReason, ThisCopy},
  state::{NewestRelease, ReadyUpdate, UpdateFailure, UpdateState},
};
use crate::{
  background,
  build::Build,
  sharing::cloud::{CloudError, address_in_use},
  workspace::Workspace,
};

/// How often the app asks for a newer release while it runs.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// The folder of the data folder downloads go to.
pub const UPDATES_FOLDER: &str = "updates";
/// How the release's installer is named, `Orchester-<version>-setup.exe` (`.github/workflows/app-release.yml`).
const INSTALLER_SUFFIX: &str = "-setup.exe";

/// This program, as its updates see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThisApp {
  /// The app's own release version.
  pub version: ReleaseVersion,
  /// The platform and architecture releases ship for; `None` on one they do not.
  pub target: Option<(ReleasePlatform, ReleaseArch)>,
  pub copy: ThisCopy,
}

impl ThisApp {
  /// This program at `version`, started as a QA copy or not.
  pub fn detect(version: ReleaseVersion, qa: bool) -> Self {
    let target = copy::this_platform().zip(ReleaseArch::parse(env::consts::ARCH));
    let program = env::current_exe()
      .map_err(|error| tracing::warn!(%error, "this program cannot tell where it is; it does not update itself"))
      .ok();
    let copy = match target {
      Some((platform, _)) => ThisCopy::of(Build::this(), qa, platform, program.as_deref().and_then(Path::parent)),
      None => ThisCopy::NotOnThisPlatform,
    };
    Self { version, target, copy }
  }
}

/// The app's updates. Clones share the state and the check in progress.
#[derive(Clone)]
pub struct AppUpdates {
  inner: Arc<Inner>,
}

struct Inner {
  workspace: Workspace,
  app: ThisApp,
  keys: AcceptedReleaseKeys,
  folder: PathBuf,
  state: watch::Sender<UpdateState>,
  /// The runs at work, which a ready update waits for.
  runs: Mutex<Vec<String>>,
  /// Held while a check, with its download, runs.
  checking: tokio::sync::Mutex<()>,
}

/// Why a check found nothing.
#[derive(Debug, thiserror::Error)]
enum CheckError {
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error(transparent)]
  Cloud(#[from] CloudError),
  #[error(transparent)]
  Client(#[from] ClientError),
  #[error("the check's database work did not finish")]
  Interrupted,
}

impl AppUpdates {
  /// The updates of `app`, whose settings are in `workspace`'s database, checked with `keys` (the release keys built
  /// into the app), downloading into [`UPDATES_FOLDER`] of `data_folder`. Nothing is asked until a check runs.
  pub fn new(workspace: Workspace, app: ThisApp, keys: AcceptedReleaseKeys, data_folder: &Path) -> Self {
    let state = match app.copy {
      // The app checks as it starts ([`keep_checking`]).
      ThisCopy::Installer => UpdateState::Checking,
      copy => not_updating(copy, None),
    };
    Self {
      inner: Arc::new(Inner {
        workspace,
        app,
        keys,
        folder: data_folder.join(UPDATES_FOLDER),
        state: watch::channel(state).0,
        runs: Mutex::default(),
        checking: tokio::sync::Mutex::new(()),
      }),
    }
  }

  pub fn this_app(&self) -> &ThisApp {
    &self.inner.app
  }

  pub fn state(&self) -> UpdateState {
    self.inner.state.borrow().clone()
  }

  /// Wakes whenever the state changed.
  pub fn subscribe(&self) -> watch::Receiver<UpdateState> {
    self.inner.state.subscribe()
  }

  /// Settings › Updates. Blocking.
  pub fn settings(&self) -> Result<UpdateSettings, StoreError> {
    self.inner.workspace.db.lock().update_settings()
  }

  /// Blocking; the next check asks for `channel`'s releases.
  pub fn set_channel(&self, channel: Channel) -> Result<(), StoreError> {
    self.inner.workspace.db.lock().set_update_channel(channel)
  }

  /// Blocking.
  pub fn set_automatic(&self, automatic: bool) -> Result<(), StoreError> {
    self.inner.workspace.db.lock().set_update_automatically(automatic)
  }

  /// When Orchester Cloud last answered a check from here. Blocking.
  pub fn last_checked(&self) -> Result<Option<Timestamp>, StoreError> {
    self.inner.workspace.db.lock().update_checked_at()
  }

  /// The runs at work now, as the person knows them (`ORC-110`): a ready update waits for them to end.
  pub fn set_runs_at_work(&self, runs: Vec<String>) {
    let mut kept = self.inner.runs.lock();
    self.inner.state.send_if_modified(|state| {
      let next = state.clone().with_runs(&runs);
      std::mem::replace(state, next) != *state
    });
    *kept = runs;
  }

  /// Asks Orchester Cloud for a newer release now and, in a copy that updates itself, downloads and checks it. A check
  /// asked for while one runs is that one.
  pub async fn check_now(&self) {
    let Ok(_running) = self.inner.checking.try_lock() else {
      return;
    };
    match (self.inner.app.copy, self.inner.app.target) {
      (ThisCopy::Installer, Some(target)) => self.update(target).await,
      (copy, Some(target)) => self.look_for_newest(copy, target).await,
      // No release ships for this platform: there is nothing to ask for.
      (_, None) => {}
    }
  }

  async fn update(&self, target: (ReleasePlatform, ReleaseArch)) {
    self.set(UpdateState::Checking);
    let (client, offered) = match self.ask(target).await {
      Ok(answer) => answer,
      Err(error) => return self.unreachable(&error).await,
    };
    let Some(release) = offered else {
      return self.up_to_date().await;
    };
    let Some(file) = installer(&release, target).cloned() else {
      let version = release.version;
      return self.set(UpdateState::Failed {
        version,
        failure: UpdateFailure::NoInstaller,
      });
    };
    let download = Download {
      folder: self.inner.folder.clone(),
      release,
      file,
      keys: self.inner.keys.clone(),
    };
    let (version, total) = (&download.release.version, download.file.size);
    let progress = |received| {
      self.set(UpdateState::Downloading {
        version: version.clone(),
        received,
        total,
      });
    };
    match download.fetch(&client, &progress).await {
      Fetched::Verified(installer) => {
        let runs = self.inner.runs.lock();
        let ready = ReadyUpdate {
          version: version.clone(),
          notes: download.release.notes.clone(),
          critical: download.release.critical,
          installer,
        };
        self.set(UpdateState::Ready(ready).with_runs(&runs));
      }
      Fetched::Failed(failure) => self.set(UpdateState::Failed {
        version: version.clone(),
        failure,
      }),
      Fetched::Interrupted(error) => self.unreachable(&error.into()).await,
    }
  }

  /// A copy that does not update itself learns the newest release; one the cloud does not answer keeps what it knew.
  async fn look_for_newest(&self, copy: ThisCopy, target: (ReleasePlatform, ReleaseArch)) {
    let newest = match self.ask(target).await {
      Ok((_, offered)) => offered.and_then(|release| newest(&release, target)),
      Err(error) => {
        tracing::info!(%error, "Orchester Cloud did not answer the update check");
        self.state().newest().cloned()
      }
    };
    self.set(not_updating(copy, newest));
  }

  /// Asks the cloud this computer uses for the release offered to this install, and keeps when it answered.
  async fn ask(
    &self,
    (platform, arch): (ReleasePlatform, ReleaseArch),
  ) -> Result<(CloudClient, Option<OfferedRelease>), CheckError> {
    let workspace = self.inner.workspace.clone();
    let (settings, install_id, cloud) = background::run(move || -> Result<_, CheckError> {
      let settings = workspace.db.lock().update_settings()?;
      Ok((settings, workspace.installation_id()?, address_in_use(&workspace)?))
    })
    .await
    .map_err(|_| CheckError::Interrupted)??;
    let client = CloudClient::new(cloud.address)?;
    let check = UpdateCheck {
      version: self.inner.app.version.clone(),
      platform,
      arch,
      channel: settings.channel,
      install_id,
    };
    let offered = client.check_for_update(&check).await?;
    let workspace = self.inner.workspace.clone();
    let kept = background::run(move || workspace.db.lock().set_update_checked_at(Timestamp::now())).await;
    if let Ok(Err(error)) = kept {
      tracing::warn!(%error, "cannot keep when the update check was answered");
    }
    Ok((client, offered))
  }

  async fn up_to_date(&self) {
    let folder = self.inner.folder.clone();
    if let Ok(Err(error)) = background::run(move || download::clear(&folder)).await {
      tracing::warn!(%error, "cannot remove older update downloads");
    }
    self.set(UpdateState::UpToDate {
      checked_at: Timestamp::now(),
    });
  }

  async fn unreachable(&self, error: &CheckError) {
    tracing::info!(%error, "Orchester Cloud did not answer the update check or the download");
    let workspace = self.inner.workspace.clone();
    let last_checked = match background::run(move || workspace.db.lock().update_checked_at()).await {
      Ok(Ok(at)) => at,
      Ok(Err(error)) => {
        tracing::warn!(%error, "cannot read when the update check was last answered");
        None
      }
      Err(_) => None,
    };
    self.set(UpdateState::CloudUnreachable { last_checked });
  }

  fn set(&self, state: UpdateState) {
    self.inner.state.send_replace(state);
  }
}

/// Checks at once and then every `every` ([`CHECK_EVERY`] in the app), for as long as the task runs.
pub async fn keep_checking(updates: AppUpdates, every: Duration) {
  loop {
    updates.check_now().await;
    tokio::time::sleep(every).await;
  }
}

/// The state of a copy that does not update itself, told of `newest`.
fn not_updating(copy: ThisCopy, newest: Option<NewestRelease>) -> UpdateState {
  match copy {
    ThisCopy::NotThisCopy(reason) => UpdateState::NotThisCopy { reason, newest },
    // An installer's copy updates itself and never gets here.
    ThisCopy::Installer | ThisCopy::NotOnThisPlatform => UpdateState::NotOnThisPlatform { newest },
  }
}

/// The release's installer for `target`.
fn installer(release: &OfferedRelease, (platform, arch): (ReleasePlatform, ReleaseArch)) -> Option<&OfferedFile> {
  release
    .files
    .iter()
    .find(|file| file.platform == platform && file.arch == arch && file.name.as_str().ends_with(INSTALLER_SUFFIX))
}

/// `release` for a copy updated by hand: its installer's download, or any file of it for `target`.
fn newest(release: &OfferedRelease, target: (ReleasePlatform, ReleaseArch)) -> Option<NewestRelease> {
  let file =
    installer(release, target).or_else(|| release.files.iter().find(|file| (file.platform, file.arch) == target))?;
  Some(NewestRelease {
    version: release.version.clone(),
    download: file.url.clone(),
  })
}
