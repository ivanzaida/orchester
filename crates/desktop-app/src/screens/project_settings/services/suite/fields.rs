//! What a suite's page holds as typed and chosen. A suite runs its program with the person's rights, as a service
//! does, so what an MCP client's input changes of what it runs (its program, arguments, filter, working folder and
//! variables) while only a person decides that is marked and put back before the next save, whoever's input saves it.

use lurq::{
  app::{component::DevtoolsInspectable, ctx::Ctx},
  core::Signal,
};
use orchester_core::resource::{ReportFormat, TestLayer};
use orchester_workspace::agent_testing::suite_editor::form::{NeedsKind, ReadFrom, SuiteForm};

use super::super::editor::fields::Lines;
use crate::mcp::input_gate::{Guarded, InputGate, input_may_decide};

/// What the page's choices hold: the report's format and where it is read from, the layer, what it needs, the
/// artifacts it keeps, whether every Test step runs it, and where among the arguments the filter's stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Choices {
  pub format: ReportFormat,
  pub read_from: ReadFrom,
  pub layer: TestLayer,
  pub needs: NeedsKind,
  pub services: Vec<String>,
  pub artifacts: Vec<String>,
  pub always: bool,
  pub filter_at: usize,
}

impl DevtoolsInspectable for Choices {}

impl Choices {
  fn of(form: &SuiteForm) -> Self {
    Self {
      format: form.format,
      read_from: form.read_from,
      layer: form.layer,
      needs: form.needs,
      services: form.services.clone(),
      artifacts: form.artifacts.clone(),
      always: form.always,
      filter_at: form.filter_at,
    }
  }
}

/// The page's fields as typed.
#[derive(Clone)]
pub(super) struct SuiteFields {
  pub name: Signal<String>,
  pub program: Signal<String>,
  pub arguments: Signal<String>,
  pub filter: Signal<String>,
  pub folder: Signal<String>,
  pub variables: Signal<Lines>,
  pub report_path: Signal<String>,
  /// The stack it needs, as *A stack*'s select holds it.
  pub stack: Signal<String>,
  /// The service *Add a service* chose, until it is added.
  pub adding: Signal<String>,
  /// The pattern typed into *Add a path pattern*, until it is added.
  pub artifact: Signal<String>,
  pub timeout: Signal<String>,
  pub choices: Signal<Choices>,
  /// An MCP client's input changed what the suite runs while only a person decides that.
  automated: Signal<bool>,
}

impl SuiteFields {
  pub(super) fn new(ctx: &mut Ctx) -> Self {
    let form = SuiteForm::default();
    Self {
      name: ctx.signal(String::new()),
      program: ctx.signal(String::new()),
      arguments: ctx.signal(String::new()),
      filter: ctx.signal(String::new()),
      folder: ctx.signal(String::new()),
      variables: ctx.signal(Lines::default()),
      report_path: ctx.signal(String::new()),
      stack: ctx.signal(String::new()),
      adding: ctx.signal(String::new()),
      artifact: ctx.signal(String::new()),
      timeout: ctx.signal(form.timeout.clone()),
      choices: ctx.signal(Choices::of(&form)),
      automated: ctx.signal(false),
    }
  }

  /// Marks what the suite runs as an MCP client's input changes it while it may not decide that.
  pub(super) fn watch_automation(&self, ctx: &mut Ctx, gate: Option<&InputGate>) {
    let mark = || {
      let (automated, gate) = (self.automated.clone(), gate.cloned());
      move || {
        if !input_may_decide(gate.as_ref(), Guarded::PermissionWidening) {
          automated.set(true);
        }
      }
    };
    for field in [&self.program, &self.arguments, &self.filter, &self.folder] {
      let changed = mark();
      ctx.watch(field, move |_: &String| changed());
    }
    let changed = mark();
    ctx.watch(&self.variables, move |_: &Lines| changed());
  }

  /// Puts what an MCP client changed of what the suite runs back to `to`'s and forgets the mark; whether it held
  /// something else.
  pub(super) fn put_back(&self, to: &SuiteForm) -> bool {
    let marked = self.automated.get_untracked();
    let now = self.form();
    let differs = now.program != to.program
      || now.arguments != to.arguments
      || now.filter != to.filter
      || now.working_folder != to.working_folder
      || now.variables != to.variables;
    let back = marked && differs;
    if back {
      self.program.set(to.program.clone());
      self.arguments.set(to.arguments.clone());
      self.filter.set(to.filter.clone());
      self.folder.set(to.working_folder.clone());
      self.variables.set(Lines(to.variables.clone()));
    }
    self.automated.set(false);
    back
  }

  pub(super) fn fill(&self, form: &SuiteForm) {
    self.name.set(form.name.clone());
    self.program.set(form.program.clone());
    self.arguments.set(form.arguments.clone());
    self.filter.set(form.filter.clone());
    self.folder.set(form.working_folder.clone());
    self.variables.set(Lines(form.variables.clone()));
    self.report_path.set(form.report_path.clone());
    self.stack.set(form.stack.clone());
    self.timeout.set(form.timeout.clone());
    self.choices.set(Choices::of(form));
  }

  /// The fields as they are now, without following them.
  pub(super) fn form(&self) -> SuiteForm {
    let choices = self.choices.get_untracked();
    SuiteForm {
      name: self.name.get_untracked(),
      program: self.program.get_untracked(),
      arguments: self.arguments.get_untracked(),
      filter: self.filter.get_untracked(),
      filter_at: choices.filter_at,
      working_folder: self.folder.get_untracked(),
      variables: self.variables.get_untracked().0,
      format: choices.format,
      read_from: choices.read_from,
      report_path: self.report_path.get_untracked(),
      layer: choices.layer,
      needs: choices.needs,
      services: choices.services,
      stack: self.stack.get_untracked(),
      artifacts: choices.artifacts,
      timeout: self.timeout.get_untracked(),
      always: choices.always,
    }
  }

  /// Follows every field, so the page renders again as one is typed in.
  pub(super) fn follow(&self) {
    for field in [
      &self.name,
      &self.program,
      &self.arguments,
      &self.filter,
      &self.folder,
      &self.report_path,
      &self.stack,
      &self.timeout,
    ] {
      field.get();
    }
    self.variables.get();
    self.choices.get();
  }

  /// Changes the choices with `change`.
  pub(super) fn choose(&self, change: impl FnOnce(&mut Choices)) {
    self.choices.update(change);
  }
}
