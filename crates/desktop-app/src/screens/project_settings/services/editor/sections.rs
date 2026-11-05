//! What the page's sections after Needs hold as typed: readiness, seed, per-run data, the UI driver and owned paths.
//! The seed runs with the person's rights like the program, so what an MCP client's input changes of it while only a
//! person decides that is marked and put back before the next save, as [`super::fields::Fields`] does for the program.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::service::UiDriver;
use orchester_workspace::agent_testing::editor::form::{
  DataForm, DataKind, ReadinessForm, ReadyKind, RequestLine, SeedForm, SeedKind, ServiceForm, UiForm,
};

use crate::mcp::input_gate::{Guarded, InputGate, input_may_decide};

/// The readiness check chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ReadyChoice(pub ReadyKind);

impl DevtoolsInspectable for ReadyChoice {}

/// The per-run data chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct DataChoice(pub DataKind);

impl DevtoolsInspectable for DataChoice {}

/// The UI driver chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DriverChoice(pub UiDriver);

impl DevtoolsInspectable for DriverChoice {}

/// The seed's kind and its requests as typed; a request's own fields type into it at its place.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Seeds {
  pub kind: SeedKind,
  pub requests: Vec<RequestLine>,
}

impl DevtoolsInspectable for Seeds {}

/// The owned path patterns, in the order they were added.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Owned(pub Vec<String>);

impl DevtoolsInspectable for Owned {}

/// The sections' fields as typed.
#[derive(Clone)]
pub(super) struct Sections {
  pub ready: Signal<ReadyChoice>,
  pub http_path: Signal<String>,
  pub log_pattern: Signal<String>,
  pub timeout: Signal<String>,
  pub seeds: Signal<Seeds>,
  pub seed_program: Signal<String>,
  pub seed_arguments: Signal<String>,
  /// The request whose fields are open, counted from 0.
  pub open_request: Signal<Option<usize>>,
  pub data: Signal<DataChoice>,
  pub data_folder: Signal<String>,
  pub driver: Signal<DriverChoice>,
  pub ui_start: Signal<String>,
  pub owned: Signal<Owned>,
  /// The pattern typed into *Add a path pattern*, until it is added.
  pub owned_adding: Signal<String>,
  /// An MCP client's input changed the seed while only a person decides that.
  automated_seed: Signal<bool>,
}

impl Sections {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let readiness = ReadinessForm::default();
    Self {
      ready: ctx.signal(ReadyChoice::default()),
      http_path: ctx.signal(String::new()),
      log_pattern: ctx.signal(String::new()),
      timeout: ctx.signal(readiness.timeout),
      seeds: ctx.signal(Seeds::default()),
      seed_program: ctx.signal(String::new()),
      seed_arguments: ctx.signal(String::new()),
      open_request: ctx.signal(None),
      data: ctx.signal(DataChoice::default()),
      data_folder: ctx.signal(String::new()),
      driver: ctx.signal(DriverChoice(UiDriver::None)),
      ui_start: ctx.signal(String::new()),
      owned: ctx.signal(Owned::default()),
      owned_adding: ctx.signal(String::new()),
      automated_seed: ctx.signal(false),
    }
  }

  /// Marks the seed as an MCP client's input changes it while it may not decide what a service runs.
  pub(super) fn watch_automation(&self, ctx: &mut Ctx, gate: Option<&InputGate>) {
    let mark = || {
      let (automated, gate) = (self.automated_seed.clone(), gate.cloned());
      move || {
        if !input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
          automated.set(true);
        }
      }
    };
    let seeds = mark();
    ctx.watch(&self.seeds, move |_: &Seeds| seeds());
    for field in [&self.seed_program, &self.seed_arguments] {
      let changed = mark();
      ctx.watch(field, move |_: &String| changed());
    }
  }

  /// Puts the seed an MCP client changed back to `to`'s and forgets the mark; whether it held something else.
  pub(super) fn put_back(&self, to: &SeedForm) -> bool {
    let marked = self.automated_seed.get_untracked();
    self.automated_seed.set(false);
    if !marked || self.seed() == *to {
      return false;
    }
    self.fill_seed(to);
    true
  }

  pub(super) fn fill(&self, form: &ServiceForm) {
    self.ready.set(ReadyChoice(form.readiness.kind));
    self.http_path.set(form.readiness.http_path.clone());
    self.log_pattern.set(form.readiness.log_pattern.clone());
    self.timeout.set(form.readiness.timeout.clone());
    self.fill_seed(&form.seed);
    self.data.set(DataChoice(form.run_data.kind));
    self.data_folder.set(form.run_data.folder.clone());
    self.driver.set(DriverChoice(form.ui.driver));
    self.ui_start.set(form.ui.start.clone());
    self.owned.set(Owned(form.owned_paths.clone()));
  }

  fn fill_seed(&self, seed: &SeedForm) {
    self.seeds.set(Seeds {
      kind: seed.kind,
      requests: seed.requests.clone(),
    });
    self.seed_program.set(seed.program.clone());
    self.seed_arguments.set(seed.arguments.clone());
  }

  fn seed(&self) -> SeedForm {
    let seeds = self.seeds.get_untracked();
    SeedForm {
      kind: seeds.kind,
      requests: seeds.requests,
      program: self.seed_program.get_untracked(),
      arguments: self.seed_arguments.get_untracked(),
    }
  }

  /// `form` with the sections as they are now, without following them.
  pub(super) fn completed(&self, form: ServiceForm) -> ServiceForm {
    ServiceForm {
      readiness: ReadinessForm {
        kind: self.ready.get_untracked().0,
        http_path: self.http_path.get_untracked(),
        log_pattern: self.log_pattern.get_untracked(),
        timeout: self.timeout.get_untracked(),
      },
      seed: self.seed(),
      run_data: DataForm {
        kind: self.data.get_untracked().0,
        folder: self.data_folder.get_untracked(),
      },
      ui: UiForm {
        driver: self.driver.get_untracked().0,
        start: self.ui_start.get_untracked(),
      },
      owned_paths: self.owned.get_untracked().0,
      ..form
    }
  }

  /// Follows every field, so the page renders again as one is typed in.
  pub(super) fn follow(&self) {
    self.ready.get();
    self.http_path.get();
    self.log_pattern.get();
    self.timeout.get();
    self.seeds.get();
    self.seed_program.get();
    self.seed_arguments.get();
    self.data.get();
    self.data_folder.get();
    self.driver.get();
    self.ui_start.get();
    self.owned.get();
  }
}
