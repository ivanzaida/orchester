use std::{io, path::PathBuf, process::ExitCode, sync::Arc};

use orchester_cloud::{Cloud, clock::SystemClock, config::Config, error::CloudError, mail::SmtpMailer};

mod shutdown;

const USAGE: &str = "usage: orchester-cloud [--config cloud.toml] [serve]";

#[tokio::main]
async fn main() -> ExitCode {
  tracing_subscriber::fmt()
    .with_env_filter(orchester_cloud::LOG_FILTER)
    .with_ansi(false)
    .with_writer(io::stderr)
    .init();
  match run().await {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      error.log();
      ExitCode::FAILURE
    }
  }
}

/// Serves until a shutdown signal: the cloud makes its root key at the first start and keeps its signing keys and key
/// sets itself, so serving is its only command.
async fn run() -> Result<(), CloudError> {
  let path = arguments()?;
  let config = Config::load(path.as_deref())?;
  let mailer = Arc::new(SmtpMailer::new(&config.smtp_url, &config.smtp_from)?);
  let listen = config.listen;
  let cloud = Cloud::open(config, Arc::new(SystemClock), mailer).await?;
  let shutdown = shutdown::Shutdown::install()?;
  let listener = std::net::TcpListener::bind(listen)?;
  tracing::info!(address = %listener.local_addr()?, "cloud listening");
  orchester_cloud::serve(cloud, listener, shutdown.wait()).await
}

/// The configuration file `--config` names, if any; the only command is `serve`, which may be left out.
fn arguments() -> Result<Option<PathBuf>, CloudError> {
  let mut args: Vec<_> = std::env::args_os().skip(1).collect();
  let config = match args.iter().position(|arg| arg == "--config") {
    Some(index) if index + 1 < args.len() => {
      let path = PathBuf::from(args.remove(index + 1));
      args.remove(index);
      Some(path)
    }
    Some(_) => return Err(CloudError::Configuration(USAGE)),
    None => None,
  };
  match args.as_slice() {
    [] => Ok(config),
    [command] if command == "serve" => Ok(config),
    _ => Err(CloudError::Configuration(USAGE)),
  }
}
