//! `repository`: a conversation where the Orchestrator's agent proposed making Kontur's folder a git repository
//! (`init_repository`, ORC-72; on the proposal cards' pattern of `ExRt1` / `k80K3d`, no frame of its own), seeded as the
//! chat keeps it, one turn holding every card (`-0` to `-4` of its action ids): waiting with a `.gitignore` the chat
//! writes and a `.env` the person decides on; waiting for only the first commit of a repository without one, the
//! folder's own `.gitignore` kept, with more files that look secret than the warning names; applied; dismissed; and
//! refused because git does not know the author, in the words the apply path gives it. Its folder is the seed's, a
//! repository with commits: Apply on a waiting card is refused there (`has_commits`).

use std::error::Error;

use lurq::app::i18n::I18n;
use orchester_core::{
  id::ChatId,
  orchestrator::{
    ActionOutcome, ChatAction, TurnState,
    change::{Change, FirstCommitOf, Gitignore, ProposedChange},
  },
  time::Timestamp,
};
use orchester_store::ProjectDb;
use orchester_workspace::{i18n::NAMESPACE, messages::Message};

use super::{
  data::{Part, Turn, ago, keep},
  proposals::elsewhere,
};

/// The `.gitignore` the chat proposes.
const GITIGNORE: &str = ".orchester/\nnode_modules/\ndist/\n*.log\n";
/// The folder's own `.gitignore`.
const KEPT: &str = "target/\n";
/// What git wrote when it did not know the author.
const SAID: &str = "Author identity unknown\n\n*** Please tell me who you are.\n\nfatal: unable to auto-detect email \
                    address (got 'ivan@DESKTOP-K2.(none)')";

/// The conversation, kept in the project at `prepared`, its refusal in the words of `i18n`.
pub fn conversation(prepared: &super::data::Prepared, i18n: &I18n) -> Result<ChatId, Box<dyn Error>> {
  let mut db = ProjectDb::open(&prepared.repo)?;
  let connection = db
    .orchestrator_settings()?
    .ok_or("the Orchestrator runs on the seed's connection")?
    .connection();
  let writes = || Change::InitRepository {
    branch: "main".to_owned(),
    first: FirstCommitOf::Files,
    files: 214,
    exists: false,
    gitignore: Some(Gitignore::Writes {
      text: GITIGNORE.to_owned(),
    }),
    sensitive: vec![".env".to_owned()],
  };
  let commit_only = Change::InitRepository {
    branch: "trunk".to_owned(),
    first: FirstCommitOf::Files,
    files: 37,
    exists: true,
    gitignore: Some(Gitignore::Kept { text: KEPT.to_owned() }),
    sensitive: [
      ".env",
      ".env.local",
      "config/credentials.json",
      "deploy/id_ed25519",
      "certs/server.key",
      "certs/server.pfx",
      ".npmrc",
    ]
    .map(str::to_owned)
    .to_vec(),
  };
  let message = Message::new("orchestrator.repository.authorUnknown").with("said", SAID.to_owned());
  let args = message.args.iter().map(|(name, value)| (*name, value.as_str()));
  let refused = ActionOutcome::Refused {
    code: "author_unknown".to_owned(),
    reason: i18n.t_ns_args(NAMESPACE, message.key, args).to_string(),
  };
  let cards = vec![
    action(writes(), ActionOutcome::Waits),
    action(commit_only, ActionOutcome::Waits),
    settled(action(writes(), ActionOutcome::Applied)),
    settled(action(writes(), ActionOutcome::Dismissed)),
    settled(action(writes(), refused)),
  ];
  let turns = vec![Turn {
    text: "Kontur isn't a git repository yet. Set it up so runs can start.",
    at: ago(20),
    parts: cards.into_iter().map(|card| Part::Action(Box::new(card))).collect(),
    end: TurnState::Answered,
  }];
  let chat = keep(&mut db, ChatId::generate(), connection, turns)?;
  elsewhere(&mut db)?;
  Ok(chat)
}

/// The chat's `init_repository` proposing `change` for Kontur, gone as `outcome`.
fn action(change: Change, outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    change: Some(Box::new(ProposedChange {
      subject: "Kontur".to_owned(),
      change,
    })),
    ..ChatAction::new("init_repository", None, outcome)
  }
}

/// `action` settled by the person now.
fn settled(action: ChatAction) -> ChatAction {
  ChatAction {
    settled: Some(Timestamp::now()),
    ..action
  }
}
