//! Where a native conversation is kept (`docs/architecture/native-agent.md` §6.2): one per step execution,
//! shared by its invocations (a resume, an answer, a restart for changed permissions), with what its
//! requests send every time and the items they grew by. Items are only appended, each before the action
//! it describes completes, so a resumed conversation sends the same bytes again.

use super::{item::Item, tools::ToolSet};
use crate::{
  connection::WireApi,
  id::{ConnectionId, ConversationId, InvocationId, RunId, StepExecutionId},
  permission::RepositoryAccess,
  resource::ModelId,
  time::Timestamp,
};

/// A native conversation, as its first invocation started it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeConversation {
  pub id: ConversationId,
  pub run: RunId,
  pub execution: StepExecutionId,
  pub connection: ConnectionId,
  pub wire: WireApi,
  pub model: ModelId,
  /// The system prompt, frozen for the conversation.
  pub system: String,
  /// Whether the step could edit files when the conversation started, which decided its tools.
  pub access: RepositoryAccess,
  pub created_at: Timestamp,
}

impl NativeConversation {
  /// The tools every request of the conversation declares.
  pub fn tools(&self) -> ToolSet {
    ToolSet::for_access(self.access)
  }
}

/// The native conversations of a project.
pub trait NativeConversationStore {
  type Error;

  /// Stores a new conversation without items.
  fn create_native_conversation(&mut self, conversation: &NativeConversation) -> Result<(), Self::Error>;

  /// The conversation `id`, if it exists.
  fn native_conversation(&self, id: ConversationId) -> Result<Option<NativeConversation>, Self::Error>;

  /// Appends `items` that `invocation` added, in order, after the conversation's last item.
  fn append_native_items(
    &mut self,
    conversation: ConversationId,
    invocation: InvocationId,
    items: &[Item],
  ) -> Result<(), Self::Error>;

  /// Every item of the conversation, in order.
  fn native_items(&self, conversation: ConversationId) -> Result<Vec<Item>, Self::Error>;
}
