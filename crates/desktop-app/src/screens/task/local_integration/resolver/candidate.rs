use lurq::core::Signal;
use orchester_core::integration::local::{EditorCandidate, Placement};

/// Candidate editing and captured-save acknowledgement; no file is resolved by this UI state.
#[derive(Clone)]
pub struct CandidateState {
  pub(super) text: Signal<String>,
  pub(super) destination: Signal<String>,
  pub(super) placement: Signal<String>,
  baseline: Signal<SavedCandidate>,
}

#[derive(Clone)]
struct SavedCandidate(EditorCandidate);
impl lurq::app::component::DevtoolsInspectable for SavedCandidate {}

impl CandidateState {
  pub fn new() -> Self {
    Self {
      text: Signal::new(String::new()),
      destination: Signal::new(String::new()),
      placement: Signal::new(String::from("unstaged")),
      baseline: Signal::new(SavedCandidate(EditorCandidate::new(
        String::new(),
        String::new(),
        Placement::LeaveUnstaged,
      ))),
    }
  }
  pub fn current(&self) -> EditorCandidate {
    EditorCandidate::new(self.text.get(), self.destination.get(), self.placement())
  }
  pub fn placement(&self) -> Placement {
    if self.placement.get() == "stage" {
      Placement::Stage
    } else {
      Placement::LeaveUnstaged
    }
  }
  pub(super) fn baseline_text(&self) -> String {
    self.baseline.get().0.text().to_owned()
  }
  pub fn dirty(&self) -> bool {
    self.current() != self.baseline.get().0
  }
  pub fn seed(&self, candidate: &EditorCandidate) {
    self.set(candidate);
    self.baseline.set(SavedCandidate(candidate.clone()));
  }
  pub fn saved(&self, captured: &EditorCandidate) {
    // Completion acknowledges exactly the captured request, never a newer candidate.
    self.baseline.set(SavedCandidate(captured.clone()));
  }
  pub fn discard(&self) {
    self.set(&self.baseline.get_untracked().0);
  }
  fn set(&self, candidate: &EditorCandidate) {
    self.text.set(candidate.text().to_owned());
    self.destination.set(candidate.destination().to_owned());
    self.placement.set(
      if candidate.placement() == Placement::Stage {
        "stage"
      } else {
        "unstaged"
      }
      .to_owned(),
    );
  }
}

impl Default for CandidateState {
  fn default() -> Self {
    Self::new()
  }
}
