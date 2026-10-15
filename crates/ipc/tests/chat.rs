//! The Orchestrator chat's messages (protocol 25): a turn asked for and stopped, the change told, the replies and
//! refusals, each read back as written; the chat's token never shows in `Debug`.

use orchester_core::id::{ChatId, ChatMessageId};
use orchester_ipc::{
  ChatToken, ChatTools, Command, Event, Rejection, Reply, Request, RequestId,
  frame::{FrameReader, FrameWriter},
};

const TOKEN: &str = "chat-token-5e1b";

#[tokio::test]
async fn chat_messages_round_trip() {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let (mut reader, mut writer) = (FrameReader::new(left), FrameWriter::new(right));
  let (chat, message) = (ChatId::generate(), ChatMessageId::generate());
  let tools = ChatTools {
    url: "http://127.0.0.1:50123/mcp".to_owned(),
    token: ChatToken::new(TOKEN.to_owned()),
  };
  let requests = [Command::AnswerInChat { message, tools }, Command::StopChat { chat }]
    .into_iter()
    .enumerate()
    .map(|(id, command)| Request {
      id: RequestId::new(u64::try_from(id).expect("small")),
      command,
    })
    .collect::<Vec<_>>();
  let reply = |reply| Event::Reply {
    id: RequestId::new(9),
    reply,
  };
  let events = vec![
    Event::ChatChanged { chat },
    reply(Reply::ChatAnswering { message }),
    reply(Reply::ChatStopped { chat }),
    reply(Reply::Rejected(Rejection::ChatMessageNotFound)),
    reply(Reply::Rejected(Rejection::ChatBusy)),
    reply(Reply::Rejected(Rejection::NoOrchestrator)),
  ];
  for request in &requests {
    writer.write(request).await.expect("write request");
  }
  for event in &events {
    writer.write(event).await.expect("write event");
  }
  drop(writer);
  for request in requests {
    let read = reader.read::<Request>().await.expect("read request");
    assert!(!format!("{read:?}").contains(TOKEN), "the token stays out of Debug");
    assert_eq!(read, Some(request));
  }
  for event in events {
    assert_eq!(reader.read::<Event>().await.expect("read event"), Some(event));
  }
}
