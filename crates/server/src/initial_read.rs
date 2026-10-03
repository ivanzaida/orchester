use std::{
  future::Future,
  io,
  pin::Pin,
  task::{Context, Poll},
  time::Duration,
};

use tokio::{
  io::{AsyncRead, AsyncWrite, ReadBuf},
  time::Sleep,
};

/// Closes a connection that sends no first byte; header/body deadlines apply once input starts.
pub(crate) struct InitialRead<T> {
  stream: T,
  deadline: Pin<Box<Sleep>>,
  started: bool,
}

impl<T> InitialRead<T> {
  pub fn new(stream: T) -> Self {
    Self {
      stream,
      deadline: Box::pin(tokio::time::sleep(Duration::from_secs(2))),
      started: false,
    }
  }
}

impl<T: AsyncRead + Unpin> AsyncRead for InitialRead<T> {
  fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buffer: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
    let this = self.get_mut();
    let before = buffer.filled().len();
    let result = Pin::new(&mut this.stream).poll_read(cx, buffer);
    if buffer.filled().len() > before {
      this.started = true;
    }
    if result.is_pending() && !this.started && this.deadline.as_mut().poll(cx).is_ready() {
      return Poll::Ready(Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "connection sent no first byte",
      )));
    }
    result
  }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for InitialRead<T> {
  fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, buffer: &[u8]) -> Poll<io::Result<usize>> {
    Pin::new(&mut self.get_mut().stream).poll_write(cx, buffer)
  }

  fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().stream).poll_flush(cx)
  }

  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
    Pin::new(&mut self.get_mut().stream).poll_shutdown(cx)
  }
}
