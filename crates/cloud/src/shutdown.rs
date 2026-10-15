use std::io;

pub(crate) struct Shutdown {
  #[cfg(unix)]
  terminate: tokio::signal::unix::Signal,
}

impl Shutdown {
  pub fn install() -> io::Result<Self> {
    #[cfg(unix)]
    {
      Ok(Self {
        terminate: tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?,
      })
    }
    #[cfg(not(unix))]
    {
      Ok(Self {})
    }
  }

  pub async fn wait(mut self) {
    #[cfg(unix)]
    tokio::select! {
      result = tokio::signal::ctrl_c() => {
        if result.is_err() { tracing::error!("shutdown signal listener failed"); }
      }
      _signal = self.terminate.recv() => {}
    }
    #[cfg(not(unix))]
    {
      // Windows has Ctrl+C; Unix additionally receives Docker's SIGTERM.
      let _shutdown = &mut self;
      if tokio::signal::ctrl_c().await.is_err() {
        tracing::error!("shutdown signal listener failed");
      }
    }
  }
}
