//! Modules of a project's tasks (ORC-48, `orchester_core::module`): the Modules list with each module's progress, a
//! module's page with its tasks and its burndown ([`burndown`]), what a module form and the tools give
//! ([`form::ModuleForm`]), the unit work is counted in ([`measure`]), and the Tasks list's module filter
//! ([`ModuleFilter`]). Modules are changed through the project's `crate::tasks::writer::TaskWriter`; a shared
//! project has none, as its tasks change on the server.

pub mod burndown;
pub mod data;
pub mod form;
pub mod measure;

use orchester_core::{id::ModuleId, task::Task};

/// Which tasks the Tasks list shows by their module.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ModuleFilter {
  #[default]
  Any,
  /// Tasks in no module.
  None,
  In(ModuleId),
}

impl ModuleFilter {
  pub fn keeps(self, task: &Task) -> bool {
    match self {
      Self::Any => true,
      Self::None => task.module().is_none(),
      Self::In(module) => task.module() == Some(module),
    }
  }
}

/// The module a task form chose for its task: none, or one of the project's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModuleChoice(pub Option<ModuleId>);
