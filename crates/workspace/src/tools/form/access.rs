//! A tool's *Access* as typed (A32 `gLLEv`, L20 `Kb6Vk`): the pipelines and steps a team run may call it from, the
//! environment whose secret values it uses (one of the secrets' three), whether a person approves a call first, how
//! long access lasts, and what a denied call does. It becomes the tool's [`AccessPolicy`], which an HTTP or database
//! tool needs.

use std::{collections::BTreeMap, num::NonZeroU32, str::FromStr};

use orchester_core::{
  id::ResourceId,
  name::Name,
  resource::{AccessPolicy, AccessPolicyError, AccessWindow, AllowedSteps, ApprovalMode, DeniedBehavior},
  secret::Environment,
};

use crate::messages::Message;

/// The fields as the user left them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessText {
  /// The pipelines a team run calls it from, with their steps; a local form keeps those it was given.
  pub callers: BTreeMap<ResourceId, AllowedSteps>,
  /// `None` while the policy names an environment that is none of the three.
  pub environment: Option<Environment>,
  pub approval: ApprovalMode,
  /// Access lasts the minutes typed, rather than until the step ends.
  pub by_minutes: bool,
  /// Whole minutes.
  pub minutes: String,
  pub denied: DeniedBehavior,
}

impl Default for AccessText {
  /// A new tool's: secrets of the environment a new secret goes in, a person approves once per run, access lasts until
  /// the step ends and a denied step waits for a new plan.
  fn default() -> Self {
    Self {
      callers: BTreeMap::new(),
      environment: Some(Environment::NEW_SECRET),
      approval: ApprovalMode::OncePerRun,
      by_minutes: false,
      minutes: String::new(),
      denied: DeniedBehavior::AwaitChangedPlan,
    }
  }
}

/// Why the policy cannot be made yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessErrors {
  pub callers: Option<Message>,
  pub environment: Option<Message>,
  pub minutes: Option<Message>,
}

impl AccessText {
  /// The fields of `policy`.
  pub fn of(policy: &AccessPolicy) -> Self {
    let (by_minutes, minutes) = match policy.window() {
      AccessWindow::UntilStepEnds => (false, String::new()),
      AccessWindow::Minutes(minutes) => (true, minutes.to_string()),
    };
    Self {
      callers: policy.callers().clone(),
      environment: environment_of(policy.environment()),
      approval: policy.approval(),
      by_minutes,
      minutes,
      denied: policy.denied(),
    }
  }

  /// The policy these fields describe.
  pub fn policy(&self) -> Result<AccessPolicy, AccessErrors> {
    let environment = self
      .environment
      .ok_or_else(|| Message::new("tools.errors.environmentRequired"));
    let window = if self.by_minutes {
      self
        .minutes
        .trim()
        .parse::<u32>()
        .ok()
        .and_then(NonZeroU32::new)
        .map(AccessWindow::Minutes)
        .ok_or_else(|| Message::new("tools.errors.minutes"))
    } else {
      Ok(AccessWindow::UntilStepEnds)
    };
    let (Ok(environment), Ok(window)) = (&environment, &window) else {
      return Err(AccessErrors {
        callers: None,
        environment: environment.err(),
        minutes: window.err(),
      });
    };
    AccessPolicy::new(
      self.callers.clone(),
      environment_name(*environment),
      self.approval,
      *window,
      self.denied,
    )
    .map_err(|AccessPolicyError::NoStepsSelected(_)| AccessErrors {
      callers: Some(Message::new("tools.errors.noStepsSelected")),
      ..AccessErrors::default()
    })
  }
}

/// The environment a policy names, ignoring case; `None` for a name that is none of the three.
pub fn environment_of(name: &Name) -> Option<Environment> {
  Environment::from_str(&name.as_str().to_lowercase()).ok()
}

/// The name a policy stores for `environment`, as the environments are written in a policy: `Production`.
pub fn environment_name(environment: Environment) -> Name {
  let name = match environment {
    Environment::Development => "Development",
    Environment::Staging => "Staging",
    Environment::Production => "Production",
  };
  Name::new(name).expect("an environment's name is a valid name")
}
