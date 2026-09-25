//! Agent profile editor (04.03 `WPmtS`) values as entered, the model and effort choices the
//! selected connection offers, and the mapping to an `AgentProfile`.

use std::collections::BTreeMap;

use orchester_core::{
  connection::{AgentConnection, ModelSupport, REASONING_EFFORT},
  name::{Name, NameError},
  resource::{AgentProfile, ModelId, PromptError, PromptText, ProviderSettings},
};

use crate::messages::{self, Message};

/// How the model is chosen: from the connection's catalog or typed in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelSelection {
  None,
  Listed(ModelId),
  Custom,
}

impl lurq::app::component::DevtoolsInspectable for ModelSelection {}

/// A model the model selector offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelOption {
  pub id: ModelId,
  pub support: ModelSupport,
}

/// The connection's catalog models, then `current` if the catalog does not list it (a model
/// that was typed in, or one the CLI no longer lists), marked as not verified.
pub fn model_options(connection: &AgentConnection, current: Option<&ModelId>) -> Vec<ModelOption> {
  let mut options: Vec<ModelOption> = connection
    .models()
    .iter()
    .map(|model| ModelOption {
      id: model.id().clone(),
      support: ModelSupport::Listed,
    })
    .collect();
  if let Some(current) = current
    && !options.iter().any(|option| &option.id == current)
  {
    options.push(ModelOption {
      id: current.clone(),
      support: ModelSupport::NotVerified,
    });
  }
  options
}

/// Reasoning efforts the connection's catalog lists for `model`; none for a model it does not
/// list, whose efforts are unknown.
pub fn effort_options<'a>(connection: &'a AgentConnection, model: &ModelId) -> &'a [String] {
  connection
    .catalog()
    .and_then(|catalog| catalog.model(model))
    .map_or(&[], |model| model.efforts())
}

/// Field values of the editor, as entered.
pub struct ProfileForm<'a> {
  pub role: &'a str,
  pub responsibilities: &'a str,
  pub instructions: &'a str,
  pub connection: Option<&'a AgentConnection>,
  pub model: &'a ModelSelection,
  /// The typed model id, used when `model` is [`ModelSelection::Custom`].
  pub custom_model: &'a str,
  /// `None` keeps the CLI's default effort.
  pub effort: Option<&'a str>,
}

/// Why the profile cannot be saved yet, per field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileErrors {
  pub role: Option<Message>,
  pub instructions: Option<Message>,
  pub connection: Option<Message>,
  pub model: Option<Message>,
}

impl ProfileForm<'_> {
  pub fn profile(&self) -> Result<AgentProfile, Box<ProfileErrors>> {
    let role = Name::new(self.role).map_err(|error| role_error(&error));
    let instructions = PromptText::new(self.instructions).map_err(|error| prompt_error(&error));
    let connection = self.connection.ok_or(Message::new("errors.connectionRequired"));
    let model = self.model_id();
    match (role, instructions, connection, model) {
      (Ok(role), Ok(instructions), Ok(connection), Ok(model)) => {
        let settings = self.settings(connection, &model);
        Ok(AgentProfile::new(
          role,
          self.responsibilities.trim().to_owned(),
          instructions,
          connection.id(),
          model,
          settings,
        ))
      }
      (role, instructions, connection, model) => Err(Box::new(ProfileErrors {
        role: role.err(),
        instructions: instructions.err(),
        connection: connection.err(),
        model: model.err(),
      })),
    }
  }

  fn model_id(&self) -> Result<ModelId, Message> {
    match self.model {
      ModelSelection::None => Err(Message::new("errors.modelRequired")),
      ModelSelection::Listed(id) => Ok(id.clone()),
      ModelSelection::Custom if self.custom_model.trim().is_empty() => Err(Message::new("errors.modelRequired")),
      ModelSelection::Custom => ModelId::new(self.custom_model.trim()).map_err(|_| Message::new("errors.modelInvalid")),
    }
  }

  /// The effort, if the model offers the chosen one. Settings the model does not offer are
  /// dropped rather than saved for the CLI to reject later.
  fn settings(&self, connection: &AgentConnection, model: &ModelId) -> ProviderSettings {
    let effort = self.effort.filter(|effort| {
      effort_options(connection, model)
        .iter()
        .any(|offered| offered == effort)
    });
    let settings = effort
      .map(|effort| BTreeMap::from([(REASONING_EFFORT.to_owned(), effort.to_owned())]))
      .unwrap_or_default();
    ProviderSettings::new(settings).expect("the reasoning effort setting name is valid")
  }
}

/// The effort a stored profile runs with, if any.
pub fn stored_effort(profile: &AgentProfile) -> Option<String> {
  profile
    .settings()
    .iter()
    .find(|(name, _)| *name == REASONING_EFFORT)
    .map(|(_, value)| value.to_owned())
}

pub fn prompt_error(error: &PromptError) -> Message {
  match error {
    PromptError::UnclosedPlaceholder { .. } => Message::new("errors.placeholderUnclosed"),
    PromptError::UnknownVariable { name, .. } => Message::new("errors.unknownVariable").with("name", name.clone()),
  }
}

fn role_error(error: &NameError) -> Message {
  match error {
    NameError::Blank => Message::new("errors.roleRequired"),
    NameError::TooLong { .. } => messages::name_error(error),
  }
}
