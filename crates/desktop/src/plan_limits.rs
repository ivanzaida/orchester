//! Desktop signal values for session readings and the project's CLI connections.
use orchester_core::connection::{AgentConnection, plan_limits::PlanReadings};

use crate::messages::Message;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Readings(pub PlanReadings);
impl lurq::app::component::DevtoolsInspectable for Readings {}

/// The project's agent connections, as the plan limits item reads them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Connections {
  /// Not read yet: the item is not shown until it is known whether the project has agents.
  #[default]
  Loading,
  Ready(Vec<AgentConnection>),
  /// They could not be read; the item says so instead of disappearing.
  Failed(Message),
}
impl lurq::app::component::DevtoolsInspectable for Connections {}
