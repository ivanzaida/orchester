//! A change of a task's content that met a newer revision (`docs/architecture/server.md`, decision
//! 26; design 10.41 and its decision 17): what the person changed is put on top of what is stored
//! now, field by field. A field only the other side changed keeps their value; a field only the
//! person changed takes theirs; a field both changed to different values is a conflict the person
//! decides. Organization fields never reach the person: they merge with
//! [`super::organization::TaskOrganization::rebase`].

use super::TaskDetails;

/// A content field of a task, as the conflict view compares them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentField {
  Title,
  Brief,
  Criteria,
  Pipeline,
  Limits,
}

impl ContentField {
  pub const ALL: [Self; 5] = [Self::Title, Self::Brief, Self::Criteria, Self::Pipeline, Self::Limits];

  fn changed(self, from: &TaskDetails, to: &TaskDetails) -> bool {
    match self {
      Self::Title => from.title != to.title,
      Self::Brief => from.brief != to.brief,
      Self::Criteria => from.acceptance_criteria != to.acceptance_criteria,
      Self::Pipeline => from.pipeline != to.pipeline,
      Self::Limits => from.limits != to.limits,
    }
  }

  /// `into` with this field taken from `from`.
  fn copy(self, from: &TaskDetails, into: &mut TaskDetails) {
    match self {
      Self::Title => into.title = from.title.clone(),
      Self::Brief => into.brief.clone_from(&from.brief),
      Self::Criteria => into.acceptance_criteria.clone_from(&from.acceptance_criteria),
      Self::Pipeline => into.pipeline = from.pipeline,
      Self::Limits => into.limits = from.limits,
    }
  }
}

/// Which version of a conflicting field the person keeps.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Choice {
  /// The person's own text, which is never dropped until they choose.
  #[default]
  Mine,
  Theirs,
}

/// Who changed a field since the person read the task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldChange {
  Unchanged,
  /// Only the person: their value is saved.
  Mine,
  /// Only the other side: their value is kept whatever the person chooses.
  Theirs,
  /// Both; to different values, a conflict the person decides.
  Both,
}

/// The person's change of a task put on top of the stored one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentMerge {
  base: TaskDetails,
  mine: TaskDetails,
  theirs: TaskDetails,
}

impl ContentMerge {
  /// `mine`, the person's edit of `base` (the task as they read it), against `theirs`, the task as
  /// it is stored now.
  pub fn new(base: TaskDetails, mine: TaskDetails, theirs: TaskDetails) -> Self {
    Self { base, mine, theirs }
  }

  /// Fields both changed, to different values: the person chooses one version of each.
  pub fn conflicts(&self) -> Vec<ContentField> {
    ContentField::ALL
      .into_iter()
      .filter(|field| {
        field.changed(&self.base, &self.mine)
          && field.changed(&self.base, &self.theirs)
          && field.changed(&self.mine, &self.theirs)
      })
      .collect()
  }

  /// Fields only the other side changed: their value is kept whatever the person chooses.
  pub fn kept(&self) -> Vec<ContentField> {
    ContentField::ALL
      .into_iter()
      .filter(|field| !field.changed(&self.base, &self.mine) && field.changed(&self.base, &self.theirs))
      .collect()
  }

  /// Whether nothing conflicts, so the change is sent again on the new revision without asking.
  pub fn is_clean(&self) -> bool {
    self.conflicts().is_empty()
  }

  /// Who changed `field`.
  pub fn change(&self, field: ContentField) -> FieldChange {
    match (
      field.changed(&self.base, &self.mine),
      field.changed(&self.base, &self.theirs),
    ) {
      (false, false) => FieldChange::Unchanged,
      (true, false) => FieldChange::Mine,
      (false, true) => FieldChange::Theirs,
      (true, true) => FieldChange::Both,
    }
  }

  /// The task as the person read it.
  pub const fn base(&self) -> &TaskDetails {
    &self.base
  }

  pub const fn mine(&self) -> &TaskDetails {
    &self.mine
  }

  pub const fn theirs(&self) -> &TaskDetails {
    &self.theirs
  }

  /// What is saved: the stored task with every field the person changed, a conflicting field as
  /// `choose` picks it.
  pub fn merged(&self, choose: impl Fn(ContentField) -> Choice) -> TaskDetails {
    let conflicts = self.conflicts();
    let mut merged = self.theirs.clone();
    for field in ContentField::ALL {
      let take_mine = if conflicts.contains(&field) {
        choose(field) == Choice::Mine
      } else {
        field.changed(&self.base, &self.mine)
      };
      if take_mine {
        field.copy(&self.mine, &mut merged);
      }
    }
    merged
  }
}
