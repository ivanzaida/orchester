//! Which copies update themselves on macOS: one that runs from an `.app` bundle (`<bundle>.app/Contents/MacOS`), not
//! moved aside by Gatekeeper's App Translocation, in a folder the person can write, so the new bundle can be renamed
//! into its place there. Read from paths alone, so it is tested on every platform; whether the bundle carries a
//! Developer ID is asked of `codesign` as the app starts (`install::bundle::signed_copy`).

use std::{
  fs::{self, OpenOptions},
  path::Path,
};

use super::{CopyReason, ThisCopy};

/// What App Translocation puts in the path of an app started from where it was downloaded, before it was moved.
const TRANSLOCATED: &str = "/AppTranslocation/";

/// The copy that runs from the program folder `folder`, on macOS.
pub(super) fn copy_of(folder: &Path) -> ThisCopy {
  let Some(bundle) = bundle_of(folder) else {
    return ThisCopy::NotThisCopy(CopyReason::NotInBundle);
  };
  if bundle.to_string_lossy().contains(TRANSLOCATED) {
    return ThisCopy::NotThisCopy(CopyReason::Translocated);
  }
  if !can_write_beside(bundle) {
    return ThisCopy::NotThisCopy(CopyReason::BundleNotWritable);
  }
  ThisCopy::Bundle
}

/// The `.app` bundle whose `Contents/MacOS` is `folder`; `None` when the program does not run from one.
pub(crate) fn bundle_of(folder: &Path) -> Option<&Path> {
  if folder.file_name()? != "MacOS" {
    return None;
  }
  let contents = folder.parent()?;
  if contents.file_name()? != "Contents" {
    return None;
  }
  let bundle = contents.parent()?;
  (bundle.extension()? == "app").then_some(bundle)
}

/// Whether a file can be made and removed in the folder that holds `bundle`, as renaming the bundle needs.
fn can_write_beside(bundle: &Path) -> bool {
  let (Some(folder), Some(name)) = (bundle.parent(), bundle.file_name()) else {
    return false;
  };
  let probe = folder.join(format!(".{}.write-check", name.to_string_lossy()));
  let written = OpenOptions::new()
    .write(true)
    .create(true)
    .truncate(true)
    .open(&probe)
    .and_then(|file| {
      drop(file);
      fs::remove_file(&probe)
    });
  if let Err(error) = written {
    tracing::info!(%error, folder = %folder.display(), "the app's folder cannot be written; it does not update itself");
    return false;
  }
  true
}
