use thiserror::Error;

use super::files::MAX_BUNDLE_FILE_BYTES;
use crate::{
  id::ResourceId,
  resource::{SkillFilePath, StepKey, VersionNumber},
  run::SnapshotError,
};

/// Why execution inputs cannot be made, or read back, as a bundle carries them.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BundleError {
  #[error(transparent)]
  Snapshot(#[from] SnapshotError),
  /// The run pins a version whose body was not given.
  #[error("the body of {resource} {number} was not given")]
  MissingVersion {
    resource: ResourceId,
    number: VersionNumber,
  },
  /// The run pins a version whose slug and name were not given.
  #[error("the label of {resource} {number} was not given")]
  MissingLabel {
    resource: ResourceId,
    number: VersionNumber,
  },
  #[error("{0} is pinned more than once")]
  Duplicate(ResourceId),
  /// The pipeline's sources are folders of one computer; a bundle never carries them.
  #[error("the pinned pipeline lists sources, which are paths of one computer")]
  PipelineSources,
  /// A profile's extra writable folders are folders of one computer; a bundle never carries them.
  #[error("an agent profile lists writable folders, which are paths of one computer")]
  WritableFolders,
  /// A command tool that starts a program by an absolute path runs only on the computer that has it.
  #[error("tool {0} starts a program by an absolute path")]
  MachinePath(ResourceId),
  /// The pinned profiles must be exactly those of the agent steps, at the versions the steps pin.
  #[error("the pinned agent profiles are not those the steps run")]
  ProfilesMismatch,
  /// A pinned profile names a team MCP server whose version was not given.
  #[error("the body of MCP server {0}, which a pinned profile names, was not given")]
  McpServerMissing(ResourceId),
  /// The pinned MCP servers must be exactly those the pinned profiles name.
  #[error("the pinned MCP servers are not those the pinned profiles name")]
  McpServersMismatch,
  #[error("step `{0}` calls a tool the inputs do not pin")]
  ToolMissing(StepKey),
  #[error(
    "file `{}` of skill {skill} has {size} bytes, more than a bundle file may have ({MAX_BUNDLE_FILE_BYTES})",
    path.as_str()
  )]
  FileTooLarge {
    skill: ResourceId,
    path: SkillFilePath,
    size: u64,
  },
}
