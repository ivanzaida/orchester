use lurq::core::Signal;
use orchester_core::integration::local::{DraftChoice, EditorCandidate, Placement};

/// Candidate editing and captured-save acknowledgement; no file is resolved by this UI state.
#[derive(Clone)]
pub struct CandidateState {
  pub(super) text: Signal<String>,
  pub(super) destination: Signal<String>,
  pub(super) placement: Signal<String>,
  baseline: Signal<SavedCandidate>,
  chosen: Signal<Chosen>,
}

#[derive(Clone)]
struct Chosen {
  choice: DraftChoice,
  text: String,
}
impl lurq::app::component::DevtoolsInspectable for Chosen {}

#[derive(Clone)]
struct SavedCandidate(EditorCandidate);
impl lurq::app::component::DevtoolsInspectable for SavedCandidate {}

impl CandidateState {
  pub fn new() -> Self {
    Self {
      text: Signal::new(String::new()),
      destination: Signal::new(String::new()),
      placement: Signal::new(String::from("unstaged")),
      chosen: Signal::new(Chosen {
        choice: DraftChoice::Text,
        text: String::new(),
      }),
      baseline: Signal::new(SavedCandidate(EditorCandidate::new(
        String::new(),
        String::new(),
        Placement::LeaveUnstaged,
      ))),
    }
  }
  pub fn current(&self) -> EditorCandidate {
    let chosen = self.chosen.get();
    let text = self.text.get();
    let choice = if chosen.text == text {
      chosen.choice
    } else {
      DraftChoice::Text
    };
    EditorCandidate::new(text, self.destination.get(), self.placement()).with_choice(choice)
  }
  pub(super) fn select(&self, choice: DraftChoice) {
    self.chosen.set(Chosen {
      choice,
      text: self.text.get(),
    });
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
    self.select(candidate.choice());
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
