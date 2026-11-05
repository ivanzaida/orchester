//! The handshake each side of a desktop–runner connection opens with: equal protocol versions pass, another version
//! and a peer that closes are told apart. Run with `cargo test -p orchester-ipc --test handshake handshake_`.

use orchester_ipc::{
  FrameError, Handshake, IpcError, PROTOCOL_VERSION,
  frame::{FrameReader, FrameWriter},
  handshake,
};
use tokio::io::{DuplexStream, ReadHalf, WriteHalf};

type Reader = FrameReader<ReadHalf<DuplexStream>>;
type Writer = FrameWriter<WriteHalf<DuplexStream>>;

/// Both ends of an in-memory connection.
fn connection() -> ((Reader, Writer), (Reader, Writer)) {
  let (left, right) = tokio::io::duplex(64 * 1024);
  let framed = |stream| {
    let (read, write) = tokio::io::split(stream);
    (FrameReader::new(read), FrameWriter::new(write))
  };
  (framed(left), framed(right))
}

#[tokio::test]
async fn handshake_succeeds_between_equal_versions() {
  let ((mut left_reader, mut left_writer), (mut right_reader, mut right_writer)) = connection();
  let (left, right) = tokio::join!(
    handshake(&mut left_reader, &mut left_writer),
    handshake(&mut right_reader, &mut right_writer),
  );
  left.expect("left handshake");
  right.expect("right handshake");
}

#[tokio::test]
async fn handshake_reports_a_version_mismatch() {
  let ((mut reader, mut writer), (mut peer_reader, mut peer_writer)) = connection();
  let peer_version = PROTOCOL_VERSION + 1;
  let peer = async {
    peer_writer
      .write(&Handshake::new(peer_version))
      .await
      .expect("peer writes its version");
    peer_reader.read::<Handshake>().await.expect("peer reads the version")
  };
  let (result, received) = tokio::join!(handshake(&mut reader, &mut writer), peer);

  assert_eq!(received, Some(Handshake::new(PROTOCOL_VERSION)));
  match result {
    Err(IpcError::VersionMismatch { local, remote }) => {
      assert_eq!(local, PROTOCOL_VERSION);
      assert_eq!(remote, peer_version);
    }
    other => panic!("expected a version mismatch, got {other:?}"),
  }
}

#[tokio::test]
async fn handshake_reports_a_peer_that_closes() {
  let ((mut reader, mut writer), peer) = connection();
  drop(peer);
  assert!(matches!(
    handshake(&mut reader, &mut writer).await,
    Err(IpcError::ClosedDuringHandshake | IpcError::Frame(FrameError::Io(_)))
  ));
}
