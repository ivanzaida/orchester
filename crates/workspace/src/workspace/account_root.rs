/// Where new agent accounts' folders go: `ORCHESTER_ACCOUNTS_DIR` when a QA copy or a test names one, so it never
/// writes the person's own account folders, else Orchester's application data (`location::accounts_dir`). The
/// installed app ignores the variable ([`crate::build::Build`]); one that is not a plain full path gives no folder.
pub(super) fn account_root() -> Option<std::path::PathBuf> {
  let named = std::env::var_os("ORCHESTER_ACCOUNTS_DIR").filter(|folder| !folder.is_empty());
  if let Some(folder) = named.filter(|_| crate::build::Build::this() == crate::build::Build::Other) {
    let folder = std::path::PathBuf::from(folder);
    if orchester_core::account::plain_full_path(&folder) {
      return Some(folder);
    }
    tracing::error!(folder = %folder.display(), "ORCHESTER_ACCOUNTS_DIR is not a plain full path; no folder for new agent accounts");
    return None;
  }
  match orchester_store::location::accounts_dir() {
    Ok(folder) => Some(folder),
    Err(error) => {
      tracing::warn!(%error, "no folder for new agent accounts");
      None
    }
  }
}
