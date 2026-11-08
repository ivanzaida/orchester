//! The form's fields the way the merge handles them: each a value of its own, so a server that runs a program and
//! one reached at an address are settled by the same rule, and the fields of the kind a merge does not make are only
//! carried along.

use super::McpField;
use crate::app_mcp::setup::{Header, NewTeamMcpServer, Runs};

/// A [`NewTeamMcpServer`] with every field set, the ones of the kind it does not run as empty.
#[derive(Clone, Debug)]
pub(super) struct MergeFields {
  name: String,
  remote: bool,
  program: String,
  arguments: Vec<String>,
  folder: String,
  variables: Vec<String>,
  url: String,
  header: Option<Header>,
  exempt_tools: Vec<String>,
}

impl MergeFields {
  pub(super) fn of(server: &NewTeamMcpServer) -> Self {
    let mut fields = Self {
      name: server.name.clone(),
      remote: false,
      program: String::new(),
      arguments: Vec::new(),
      folder: String::new(),
      variables: Vec::new(),
      url: String::new(),
      header: None,
      exempt_tools: server.exempt_tools.clone(),
    };
    match &server.runs {
      Runs::Program {
        program,
        arguments,
        folder,
        variables,
      } => {
        fields.program.clone_from(program);
        fields.arguments.clone_from(arguments);
        fields.folder.clone_from(folder);
        fields.variables.clone_from(variables);
      }
      Runs::Remote { url, header } => {
        fields.remote = true;
        fields.url.clone_from(url);
        fields.header.clone_from(header);
      }
    }
    fields
  }

  /// The value of `field` as the words it is shown in; empty for none.
  pub(super) fn value(&self, field: McpField) -> Vec<String> {
    let one = |text: &str| {
      if text.is_empty() {
        Vec::new()
      } else {
        vec![text.to_owned()]
      }
    };
    match field {
      McpField::Name => one(&self.name),
      McpField::RunsAs => vec![if self.remote { "remote" } else { "program" }.to_owned()],
      McpField::Program => one(&self.program),
      McpField::Arguments => self.arguments.clone(),
      McpField::Folder => one(&self.folder),
      McpField::Variables => self.variables.clone(),
      McpField::Url => one(&self.url),
      McpField::Header => self.header.iter().map(header_words).collect(),
      McpField::ExemptTools => self.exempt_tools.clone(),
    }
  }

  /// Whether `field` is part of how this server runs.
  pub(super) const fn applies(&self, field: McpField) -> bool {
    match field {
      McpField::Name | McpField::RunsAs | McpField::ExemptTools => true,
      McpField::Program | McpField::Arguments | McpField::Folder | McpField::Variables => !self.remote,
      McpField::Url | McpField::Header => self.remote,
    }
  }

  /// Sets `field` to what `from` holds.
  pub(super) fn take(&mut self, field: McpField, from: &Self) {
    match field {
      McpField::Name => self.name.clone_from(&from.name),
      McpField::RunsAs => self.remote = from.remote,
      McpField::Program => self.program.clone_from(&from.program),
      McpField::Arguments => self.arguments.clone_from(&from.arguments),
      McpField::Folder => self.folder.clone_from(&from.folder),
      McpField::Variables => self.variables.clone_from(&from.variables),
      McpField::Url => self.url.clone_from(&from.url),
      McpField::Header => self.header.clone_from(&from.header),
      McpField::ExemptTools => self.exempt_tools.clone_from(&from.exempt_tools),
    }
  }

  pub(super) fn into_server(self) -> NewTeamMcpServer {
    let runs = if self.remote {
      Runs::Remote {
        url: self.url,
        header: self.header,
      }
    } else {
      Runs::Program {
        program: self.program,
        arguments: self.arguments,
        folder: self.folder,
        variables: self.variables,
      }
    };
    NewTeamMcpServer {
      name: self.name,
      runs,
      exempt_tools: self.exempt_tools,
    }
  }
}

/// The header as the words a row shows it in; the prefix is told apart from the secret's name.
fn header_words(header: &Header) -> String {
  if header.prefix.is_empty() {
    format!("{} ← {}", header.name, header.secret)
  } else {
    format!("{} ← {} (prefix {:?})", header.name, header.secret, header.prefix)
  }
}
