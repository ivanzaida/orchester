use super::{Checkout, LocalError, LocalOperation, LocalPhase, Preservation};

impl LocalOperation {
  pub fn begin_preserve(&mut self, current: &Checkout, name: &str) -> Result<(), LocalError> {
    let paths = current.files.iter().map(|file| file.path.clone()).collect();
    self.begin_preserve_selected(current, name, paths)
  }

  pub fn begin_preserve_selected(
    &mut self,
    current: &Checkout,
    name: &str,
    paths: Vec<crate::integration::local::LocalPath>,
  ) -> Result<(), LocalError> {
    self.prepare_phase(current)?;
    if name.trim().is_empty() || paths.is_empty() {
      return Err(LocalError::Blank);
    }
    if paths.iter().collect::<std::collections::BTreeSet<_>>().len() != paths.len() {
      return Err(LocalError::Stale);
    }
    let files = paths
      .iter()
      .map(|path| {
        current
          .files
          .iter()
          .find(|file| file.path == *path)
          .cloned()
          .ok_or(LocalError::Stale)
      })
      .collect::<Result<Vec<_>, _>>()?;
    if let Some(saved) = &mut self.preservation {
      if !saved.verified || !saved.retained || saved.original.head != current.head {
        return Err(LocalError::WrongPhase);
      }
      if files
        .iter()
        .any(|file| saved.original.files.iter().any(|original| original.path == file.path))
      {
        return Err(LocalError::WrongPhase);
      }
      saved.original.files.extend(files);
      saved.name = name.trim().to_owned();
      saved.verified = false;
    } else {
      let mut original = current.clone();
      original.files = files;
      self.preservation = Some(Preservation {
        name: name.trim().to_owned(),
        original,
        verified: false,
        retained: true,
      });
    }
    self.preservation_selection = paths;
    self.phase = LocalPhase::Preserving;
    Ok(())
  }

  pub fn preservation_selection(&self) -> Vec<crate::integration::local::LocalPath> {
    if self.preservation_selection.is_empty() {
      // Journals written before selected preservation retained every original path.
      self.preservation.as_ref().map_or_else(Vec::new, |saved| {
        saved.original.files.iter().map(|file| file.path.clone()).collect()
      })
    } else {
      self.preservation_selection.clone()
    }
  }

  pub fn preservation_finished(&mut self, current: Checkout) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Preserving || !self.preservation.as_ref().is_some_and(|saved| saved.verified) {
      return Err(LocalError::NotVerified);
    }
    self.reviewed = current;
    self.preservation_selection.clear();
    self.phase = if self.reviewed.clean() {
      LocalPhase::Prepared
    } else {
      LocalPhase::Review
    };
    Ok(())
  }

  pub fn preservation_verified(&mut self) -> Result<(), LocalError> {
    if self.phase != LocalPhase::Preserving {
      return Err(LocalError::WrongPhase);
    }
    self.preservation.as_mut().ok_or(LocalError::NotVerified)?.verified = true;
    Ok(())
  }
}
