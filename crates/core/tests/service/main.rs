//! Services and stacks: the invariants their constructors keep, and how needs resolve into start steps.

mod definition;
mod needs;
mod seed;

use orchester_core::{
  resource::CommandProgram,
  service::{Readiness, ReadyCheck, ReadyTimeout, Service, ServiceName},
};

pub fn name(value: &str) -> ServiceName {
  ServiceName::new(value).expect("a kebab-case name")
}

pub fn service(value: &str, needs: &[&str]) -> Service {
  let readiness = Readiness::new(ReadyCheck::PortOpen, ReadyTimeout::from_secs(30).expect("in range"));
  let program = CommandProgram::new("npm").expect("a program on the PATH");
  Service::new(name(value), program, readiness)
    .with_needs(needs.iter().map(|need| name(need)).collect())
    .expect("needs other services once each")
}
