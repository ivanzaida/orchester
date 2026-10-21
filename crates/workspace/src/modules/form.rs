//! A module's details as a form and the tools give them: a name, a description, and dates written `YYYY-MM-DD` or
//! left empty.

use chrono::NaiveDate;
use orchester_core::{
  module::{ModuleDetails, ModuleState},
  name::Name,
};

use crate::messages::{self, Message};

/// How dates are written.
pub const DATE_FORMAT: &str = "%Y-%m-%d";

/// What was typed.
#[derive(Clone, Copy, Debug)]
pub struct ModuleForm<'a> {
  pub name: &'a str,
  pub description: &'a str,
  pub start: &'a str,
  pub target: &'a str,
  pub state: ModuleState,
}

/// Why the form's details are not accepted, field by field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleFormErrors {
  pub name: Option<Message>,
  pub start: Option<Message>,
  pub target: Option<Message>,
}

impl ModuleFormErrors {
  /// Every reason, in the form's order.
  pub fn all(self) -> Vec<Message> {
    [self.name, self.start, self.target].into_iter().flatten().collect()
  }
}

impl ModuleForm<'_> {
  pub fn details(&self) -> Result<ModuleDetails, ModuleFormErrors> {
    let name = Name::new(self.name).map_err(|error| messages::name_error(&error));
    let (start, target) = (day(self.start), day(self.target));
    let (Ok(name), Ok(start), Ok(target)) = (&name, &start, &target) else {
      return Err(ModuleFormErrors {
        name: name.err(),
        start: start.err(),
        target: target.err(),
      });
    };
    ModuleDetails::new(name.clone(), self.description, (*start, *target), self.state).map_err(|error| {
      ModuleFormErrors {
        target: Some(messages::module_error(&error)),
        ..ModuleFormErrors::default()
      }
    })
  }
}

/// A day written `YYYY-MM-DD`; empty for none.
pub fn day(text: &str) -> Result<Option<NaiveDate>, Message> {
  let text = text.trim();
  if text.is_empty() {
    return Ok(None);
  }
  NaiveDate::parse_from_str(text, DATE_FORMAT)
    .map(Some)
    .map_err(|_| Message::new("modules.errors.invalidDate"))
}
