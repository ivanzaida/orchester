//! A command tool's editor tabs, preserved in the project's Tools route.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ToolTab {
  #[default]
  Configuration,
  InputsOutputs,
  Access,
  UsageLog,
}
impl ToolTab {
  pub const ALL: [Self; 4] = [Self::Configuration, Self::InputsOutputs, Self::Access, Self::UsageLog];
  pub const fn name(self) -> &'static str {
    match self {
      Self::Configuration => "configuration",
      Self::InputsOutputs => "inputs_outputs",
      Self::Access => "access",
      Self::UsageLog => "usage_log",
    }
  }
  pub fn from_name(name: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|tab| tab.name() == name)
  }
}
