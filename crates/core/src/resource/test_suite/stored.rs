//! The JSON shape of a test suite's body. Reading it runs every constructor again, so a stored body
//! that breaks a rule is refused instead of loaded.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
  ArtifactGlob, ReportFormat, SuiteCommand, SuiteNeeds, SuitePath, SuiteVariable, SuiteVariables, TestLayer,
  TestReport, TestSuite, TestSuiteError,
};
use crate::{
  resource::{CommandArg, CommandProgram, CommandTimeout, Slug, VariableName},
  secret::SecretName,
};

#[derive(Serialize, Deserialize)]
struct Fields {
  program: String,
  args: Vec<String>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  working_folder: Option<String>,
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  variables: BTreeMap<String, VariableFields>,
  report: ReportFields,
  layer: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  needs: Vec<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  artifacts: Vec<String>,
  timeout_secs: u64,
  always: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum VariableFields {
  Plain(String),
  Secret(String),
}

#[derive(Serialize, Deserialize)]
struct ReportFields {
  format: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  path: Option<String>,
}

impl Serialize for TestSuite {
  fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    Fields::from(self).serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for TestSuite {
  fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let fields = Fields::deserialize(deserializer)?;
    Self::try_from(fields).map_err(serde::de::Error::custom)
  }
}

impl From<&TestSuite> for Fields {
  fn from(suite: &TestSuite) -> Self {
    let variables = suite
      .variables
      .iter()
      .map(|(name, value)| {
        let value = match value {
          SuiteVariable::Plain(plain) => VariableFields::Plain(plain.clone()),
          SuiteVariable::Secret(secret) => VariableFields::Secret(secret.as_str().to_owned()),
        };
        (name.as_str().to_owned(), value)
      })
      .collect();
    Self {
      program: suite.command.program().as_str().to_owned(),
      args: suite.command.args().iter().map(|arg| arg.as_str().to_owned()).collect(),
      working_folder: suite.working_folder.as_ref().map(|folder| folder.as_str().to_owned()),
      variables,
      report: ReportFields {
        format: suite.report.format().as_str().to_owned(),
        path: suite.report.path().map(|path| path.as_str().to_owned()),
      },
      layer: suite.layer.as_str().to_owned(),
      needs: suite
        .needs
        .as_slice()
        .iter()
        .map(|need| need.as_str().to_owned())
        .collect(),
      artifacts: suite.artifacts.iter().map(|glob| glob.as_str().to_owned()).collect(),
      timeout_secs: suite.timeout.secs(),
      always: suite.always,
    }
  }
}

impl TryFrom<Fields> for TestSuite {
  type Error = TestSuiteError;

  fn try_from(fields: Fields) -> Result<Self, TestSuiteError> {
    let layer = TestLayer::parse(&fields.layer)?;
    let args = fields
      .args
      .iter()
      .map(|arg| CommandArg::new(arg))
      .collect::<Result<_, _>>()?;
    let command = SuiteCommand::new(CommandProgram::new(&fields.program)?, args)?;
    let report = TestReport::new(
      ReportFormat::parse(&fields.report.format)?,
      fields.report.path.as_deref().map(SuitePath::new).transpose()?,
    )?;
    let needs = fields
      .needs
      .iter()
      .map(|need| Slug::new(need))
      .collect::<Result<_, _>>()?;
    let artifacts = fields
      .artifacts
      .iter()
      .map(|glob| ArtifactGlob::new(glob))
      .collect::<Result<_, _>>()?;
    Ok(
      Self::new(command, report, layer)
        .with_working_folder(fields.working_folder.as_deref().map(SuitePath::new).transpose()?)
        .with_variables(variables(fields.variables)?)
        .with_needs(SuiteNeeds::new(needs)?)
        .with_artifacts(artifacts)
        .with_timeout(CommandTimeout::from_secs(fields.timeout_secs)?)
        .run_always(fields.always),
    )
  }
}

fn variables(fields: BTreeMap<String, VariableFields>) -> Result<SuiteVariables, TestSuiteError> {
  let variables = fields
    .into_iter()
    .map(|(name, value)| {
      let value = match value {
        VariableFields::Plain(plain) => SuiteVariable::Plain(plain),
        VariableFields::Secret(secret) => SuiteVariable::Secret(SecretName::new(&secret)?),
      };
      Ok((VariableName::new(&name)?, value))
    })
    .collect::<Result<_, TestSuiteError>>()?;
  SuiteVariables::new(variables)
}
