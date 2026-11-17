//! How a module with a branch of its own reaches its base branch (`docs/plan/orchestrator-autonomy/OA-17-…`,
//! `OA-20-…`, `OA-32-…`): its tasks merge into its branch, an acceptance looks at the whole of it, and its delivery
//! merges the branch into the base. One rule for a local module and a team module, which differ only in who keeps the
//! data and who authorizes ([`DeliveryFacts::step`]). A pure rule: what it reads is read from the project by its
//! caller.

use serde::{Deserialize, Serialize};

use crate::{
  id::{ModuleId, ResourceId},
  name::Name,
  revision::Revision,
  run::BranchName,
  time::Timestamp,
};

/// How many acceptances of one module the conversation that leads it starts by itself; a further one is the person's.
pub const ACCEPTANCES_MOST: usize = 2;

/// A module's own branch: its tasks' runs start from it and merge into it, and the module reaches its base branch as
/// one whole, by its delivery.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnBranch {
  /// `module/<slug>`, kept once chosen: renaming the module does not rename it.
  pub branch: BranchName,
  /// The pipeline the module's acceptance runs on; `None`: it has none, and only the person delivers it.
  pub acceptance: Option<ResourceId>,
  /// The conversation that leads the module to the end may deliver it once its acceptance passed.
  pub deliver_when_accepted: bool,
}

impl OwnBranch {
  /// The branch `branch` with no acceptance pipeline, delivered by the person.
  pub const fn new(branch: BranchName) -> Self {
    Self {
      branch,
      acceptance: None,
      deliver_when_accepted: false,
    }
  }
}

/// The branch a module named `name` gets: `module/<slug>`, the slug its name in lower-case letters, digits and
/// dashes, with `-2`, `-3`, … after it while `taken` says the name is in use. `None` when no such name is a branch
/// name git takes (which no slug of these characters is).
pub fn own_branch_name(name: &Name, taken: impl Fn(&BranchName) -> bool) -> Option<BranchName> {
  let mut slug = String::new();
  for letter in name.as_str().chars().flat_map(char::to_lowercase) {
    if letter.is_ascii_alphanumeric() {
      slug.push(letter);
    } else if !slug.ends_with('-') && !slug.is_empty() {
      slug.push('-');
    }
  }
  let slug = slug.trim_end_matches('-');
  let slug = if slug.is_empty() { "module" } else { slug };
  (1_u32..)
    .map(|number| match number {
      1 => format!("module/{slug}"),
      number => format!("module/{slug}-{number}"),
    })
    .map(|named| BranchName::new(&named).ok())
    .take(1000)
    .find_map(|branch| branch.filter(|branch| !taken(branch)))
}

/// A delivery of a module that merged, as the project keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleDelivered {
  pub module: ModuleId,
  /// The commit of the module's branch that was delivered.
  pub head: Revision,
  /// The base branch it was merged into.
  pub into: BranchName,
  /// The commit that branch is at since.
  pub merge: Revision,
  pub at: Timestamp,
}

/// Where the latest acceptance of a module stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Acceptance {
  /// It had none.
  None,
  /// Its acceptance task is not done yet.
  UnderWay,
  /// It passed, and nothing was merged into the module's branch since.
  Passed,
  /// It did not pass, or work was merged into the module's branch after it.
  NotPassed,
}

/// What the rule reads of a module whose every task is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeliveryFacts {
  /// It has a branch of its own.
  pub own_branch: bool,
  /// It names an acceptance pipeline.
  pub acceptance_pipeline: bool,
  /// The conversation that leads it may deliver it once accepted.
  pub deliver_when_accepted: bool,
  /// How many acceptances it had.
  pub acceptances: usize,
  pub acceptance: Acceptance,
  /// Its branch, as it is now, was delivered, or nothing was ever merged into it.
  pub delivered: bool,
}

/// Why a module's delivery is the person's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersonsDelivery {
  /// The module names no acceptance pipeline: nothing looked at the whole of it.
  NoAcceptance,
  /// Its acceptances did not pass, as many as the conversation starts by itself.
  AcceptanceCeiling,
  /// It is accepted, and *Deliver when accepted* is off for it.
  NotAllowed,
  /// A team module's delivery was asked of its team server, which decides it: it waits for an Admin's authorization,
  /// after which it is asked for again.
  TeamDecides,
  /// A team module's delivery was refused by its team server, or its merge failed.
  TeamRefused,
}

/// What a module whose every task is done waits for before it is on its base branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryStep {
  /// Nothing: it has no branch of its own, or its branch was delivered.
  Delivered,
  /// Its acceptance, which the conversation that leads it starts.
  Accept,
  /// Its acceptance is under way.
  Accepting,
  /// Its delivery, which the conversation that leads it asks for.
  Deliver,
  /// Its delivery, or what comes before it, is the person's.
  Person(PersonsDelivery),
}

impl DeliveryFacts {
  /// What the module waits for now.
  pub const fn step(&self) -> DeliveryStep {
    if !self.own_branch || self.delivered {
      return DeliveryStep::Delivered;
    }
    match self.acceptance {
      Acceptance::Passed if self.deliver_when_accepted => DeliveryStep::Deliver,
      Acceptance::Passed => DeliveryStep::Person(PersonsDelivery::NotAllowed),
      Acceptance::UnderWay => DeliveryStep::Accepting,
      Acceptance::None | Acceptance::NotPassed if !self.acceptance_pipeline => {
        DeliveryStep::Person(PersonsDelivery::NoAcceptance)
      }
      Acceptance::None | Acceptance::NotPassed if self.acceptances >= ACCEPTANCES_MOST => {
        DeliveryStep::Person(PersonsDelivery::AcceptanceCeiling)
      }
      Acceptance::None | Acceptance::NotPassed => DeliveryStep::Accept,
    }
  }
}

impl DeliveryStep {
  /// The name it is told by.
  pub const fn name(self) -> &'static str {
    match self {
      Self::Delivered => "delivered",
      Self::Accept => "accept",
      Self::Accepting => "accepting",
      Self::Deliver => "deliver",
      Self::Person(PersonsDelivery::NoAcceptance) => "person_no_acceptance",
      Self::Person(PersonsDelivery::AcceptanceCeiling) => "person_acceptance_ceiling",
      Self::Person(PersonsDelivery::NotAllowed) => "person_not_allowed",
      Self::Person(PersonsDelivery::TeamDecides) => "person_team_decides",
      Self::Person(PersonsDelivery::TeamRefused) => "person_team_refused",
    }
  }
}
