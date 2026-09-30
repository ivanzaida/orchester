//! Desktop signal values for session readings and the project's CLI connections.
use orchester_core::connection::{AgentConnection, plan_limits::PlanReadings};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Readings(pub PlanReadings);
impl lurq::app::component::DevtoolsInspectable for Readings {}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Connections(pub Vec<AgentConnection>);
impl lurq::app::component::DevtoolsInspectable for Connections {}
