//! The new bundle, unpacked from the release's zip into a hidden folder beside the running bundle, so the helper's
//! renames stay on one volume.

use std::{
  fs, io,
  path::{Path, PathBuf},
  process::Command,
};

use super::run;

const DITTO: &str = "/usr/bin/ditto";

/// The new bundle, unpacked.
pub(super) struct Unpacked {
  /// The hidden folder it is in, `<folder>/.<bundle name>.partial`.
  pub folder: PathBuf,
  /// The new bundle in it.
  pub app: PathBuf,
}

impl Unpacked {
  /// Unpacks `zip` beside the bundle `installed`, over what an earlier install left there.
  pub fn from_zip(zip: &Path, installed: &Path) -> io::Result<Self> {
    let (Some(parent), Some(name)) = (installed.parent(), installed.file_name()) else {
      return Err(io::Error::other("the app's bundle has no folder"));
    };
    let folder = parent.join(format!(".{}.partial", name.to_string_lossy()));
    remove_folder(&folder)?;
    run(Command::new(DITTO).args(["-x", "-k"]).arg(zip).arg(&folder))
      .map_err(|error| io::Error::other(format!("cannot unpack the update: {error}")))?;
    match the_app(&folder) {
      Ok(app) => Ok(Self { folder, app }),
      Err(error) => {
        Self {
          folder,
          app: PathBuf::new(),
        }
        .remove();
        Err(error)
      }
    }
  }

  /// Removes what was unpacked. A failure is only logged: the next install removes it before it unpacks.
  pub fn remove(&self) {
    if let Err(error) = remove_folder(&self.folder) {
      tracing::warn!(%error, folder = %self.folder.display(), "cannot remove the unpacked update");
    }
  }
}

/// The one `.app` bundle in `folder`, as the release's zip holds it.
fn the_app(folder: &Path) -> io::Result<PathBuf> {
  let mut apps = Vec::new();
  for entry in fs::read_dir(folder)? {
    let path = entry?.path();
    if path.extension().is_some_and(|extension| extension == "app") && path.is_dir() {
      apps.push(path);
    }
  }
  match <[PathBuf; 1]>::try_from(apps) {
    Ok([app]) => Ok(app),
    Err(apps) => Err(io::Error::other(format!(
      "the update holds {} app bundles instead of one",
      apps.len()
    ))),
  }
}

fn remove_folder(folder: &Path) -> io::Result<()> {
  match fs::remove_dir_all(folder) {
    Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
    _ => Ok(()),
  }
}
