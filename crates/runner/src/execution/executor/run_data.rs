//! A run's per-run data when the run ends, and what a runner that died left (`orchester_core::service::run_data`).
//!
//! When a run ends (it succeeded, failed or was cancelled), every container its storage still notes is removed, and
//! its data folder with its services' run folders goes. A service that is still being stopped may hold a file there a
//! moment longer (Windows refuses to delete it meanwhile), so the folder is tried again a few times.
//!
//! When the runner starts, no service of its project runs yet: one runner per project. So a container its runs'
//! storage notes, that still carries the project's label, is what a runner that died left, and it is removed; the
//! notes are read before the runner starts anything, and a start names its container by its port, so none is a
//! container started since. The data folders of runs that ended meanwhile go too. Both run on a thread of their own:
//! Docker and a folder full of a database's files take their time. Without Docker the notes stay for the next start.

use std::{
  fs, io,
  path::{Path, PathBuf},
  thread,
  time::Duration,
};

use orchester_core::{
  id::RunId,
  service::{CONTAINER_PROJECT_LABEL, ContainerName, container_notes_folder, run_data_folder},
  store::RunStore,
  workspaces::ARTIFACTS_DIR,
};
use orchester_exec::{docker::Docker, search_path::search_path};
use orchester_store::ProjectDb;

/// How often deleting a run's data folder is tried.
const DELETE_TRIES: u32 = 10;
/// How long a try waits after one that failed.
const DELETE_PAUSE: Duration = Duration::from_millis(500);

/// A container a run's storage notes: its name and the note.
struct Noted {
  name: ContainerName,
  note: PathBuf,
}

/// Removes, on a thread of its own, what run `run` of the project at `root` kept per run: its containers and its data
/// folder.
pub(in crate::execution) fn end_in_background(root: PathBuf, run: RunId) {
  spawn("run-data-end", move || {
    let noted = noted(&root, run);
    if !noted.is_empty() {
      match Docker::find(search_path().as_deref()) {
        Ok(docker) => {
          for container in &noted {
            remove(&docker, container);
          }
        }
        Err(error) => tracing::warn!(%error, %run, "the run's containers stay until the runner starts again"),
      }
    }
    delete_with_tries(&run_data_folder(&root, run));
  });
}

/// Removes, on a thread of its own, what the runs of the project at `root` left when a runner died: the containers
/// their storage notes now that carry the project's label, and the data folders of runs that ended.
pub(in crate::execution) fn sweep_in_background(root: PathBuf) {
  let runs = stored_runs(&root);
  let noted: Vec<Noted> = runs.iter().flat_map(|run| noted(&root, *run)).collect();
  spawn("run-data-sweep", move || {
    let db = match ProjectDb::open_read_only(&root) {
      Ok(db) => db,
      Err(error) => {
        tracing::warn!(%error, "cannot read the project's runs; their per-run data stays for the next start");
        return;
      }
    };
    if !noted.is_empty() {
      remove_left(&db, &noted);
    }
    for run in runs {
      let ended = match db.run(run) {
        Ok(found) => found.is_none_or(|found| found.status().is_terminal()),
        Err(error) => {
          tracing::warn!(%error, %run, "cannot read a run; its per-run data stays");
          false
        }
      };
      if ended {
        delete_with_tries(&run_data_folder(&root, run));
      }
    }
  });
}

/// Removes each of `noted` that still carries the label of the project `db` holds.
fn remove_left(db: &ProjectDb, noted: &[Noted]) {
  let docker = match Docker::find(search_path().as_deref()) {
    Ok(docker) => docker,
    Err(error) => {
      tracing::warn!(%error, "the containers a runner left stay until Docker is there");
      return;
    }
  };
  let project = db.project().id().to_string();
  let labelled = match docker.labelled(CONTAINER_PROJECT_LABEL, &project) {
    Ok(labelled) => labelled,
    Err(error) => {
      tracing::warn!(%error, "the containers a runner left cannot be listed; they stay for the next start");
      return;
    }
  };
  for container in noted {
    if labelled.iter().any(|name| name == container.name.as_str()) {
      remove(&docker, container);
    } else {
      forget(container);
    }
  }
}

/// Removes `container`, then its note.
fn remove(docker: &Docker, container: &Noted) {
  match docker.remove(container.name.as_str()) {
    Ok(()) => {
      tracing::info!(container = container.name.as_str(), "removed a container a run left");
      forget(container);
    }
    Err(error) => tracing::warn!(%error, container = container.name.as_str(), "a container a run left stays"),
  }
}

/// Deletes the note of `container`, which is gone.
fn forget(container: &Noted) {
  if let Err(error) = fs::remove_file(&container.note)
    && error.kind() != io::ErrorKind::NotFound
  {
    tracing::warn!(%error, note = %container.note.display(), "a container's note stays");
  }
}

/// The containers the storage of run `run` notes, by the names Orchester gives them.
fn noted(root: &Path, run: RunId) -> Vec<Noted> {
  let folder = container_notes_folder(root, run);
  let entries = match fs::read_dir(&folder) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Vec::new(),
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "a run's container notes cannot be read");
      return Vec::new();
    }
  };
  entries
    .filter_map(Result::ok)
    .filter_map(|entry| {
      let name = ContainerName::noted(run, entry.file_name().to_str()?)?;
      Some(Noted {
        name,
        note: entry.path(),
      })
    })
    .collect()
}

/// The runs whose storage is in the project at `root`.
fn stored_runs(root: &Path) -> Vec<RunId> {
  let folder = root.join(ARTIFACTS_DIR);
  match fs::read_dir(&folder) {
    Ok(entries) => entries
      .filter_map(Result::ok)
      .filter_map(|entry| entry.file_name().to_str()?.parse().ok())
      .collect(),
    Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
    Err(error) => {
      tracing::warn!(%error, folder = %folder.display(), "the runs' storage cannot be read");
      Vec::new()
    }
  }
}

/// Deletes `folder` and everything in it, trying again while something still holds a file there.
fn delete_with_tries(folder: &Path) {
  for attempt in 1..=DELETE_TRIES {
    match fs::remove_dir_all(folder) {
      Ok(()) => return,
      Err(error) if error.kind() == io::ErrorKind::NotFound => return,
      Err(error) if attempt == DELETE_TRIES => {
        tracing::warn!(%error, folder = %folder.display(), "a run's data folder stays until the runner starts again");
      }
      Err(_) => thread::sleep(DELETE_PAUSE),
    }
  }
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
  if let Err(error) = thread::Builder::new().name(name.to_owned()).spawn(work) {
    tracing::warn!(%error, "cannot start the cleanup of runs' per-run data");
  }
}
