use std::{
  io::{self, Write},
  path::{Path, PathBuf},
  process::ExitCode,
  sync::Arc,
};

use orchester_cloud::{Cloud, clock::SystemClock, config::Config, error::CloudError, mail::SmtpMailer};

mod ceremony;
mod shutdown;

/// Commands that run where the root key is: no configuration, database or network.
enum Offline {
  RootGenerate(PathBuf),
  KeysSign {
    statement: PathBuf,
    root: PathBuf,
    signed: PathBuf,
  },
}

/// Commands that run where the cloud runs, against its configuration and database.
enum Online {
  Serve,
  KeysRotate,
  KeysPrepare(PathBuf),
  KeysPublish(PathBuf),
}

enum Command {
  Offline(Offline),
  Online(Online),
}

const USAGE: &str = "usage: orchester-cloud [--config cloud.toml] [serve | keys rotate | keys prepare <statement> | \
                     keys publish <signed>]\n       orchester-cloud root generate <root-key> | keys sign <statement> \
                     <root-key> <signed>";

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

async fn run() -> Result<(), CloudError> {
  match arguments()? {
    (Command::Offline(Offline::RootGenerate(root)), _) => ceremony::generate_root(&root),
    (
      Command::Offline(Offline::KeysSign {
        statement,
        root,
        signed,
      }),
      _,
    ) => ceremony::sign(&statement, &root, &signed),
    (Command::Online(command), path) => run_online(command, path.as_deref()).await,
  }
}

async fn run_online(command: Online, path: Option<&Path>) -> Result<(), CloudError> {
  let config = Config::load(path)?;
  let mailer = Arc::new(SmtpMailer::new(&config.smtp_url, &config.smtp_from)?);
  let listen = config.listen;
  let cloud = Cloud::open(config, Arc::new(SystemClock), mailer).await?;
  match command {
    Online::Serve => {
      let shutdown = shutdown::Shutdown::install()?;
      let listener = std::net::TcpListener::bind(listen)?;
      tracing::info!(address = %listener.local_addr()?, "cloud listening");
      orchester_cloud::serve(cloud, listener, shutdown.wait()).await
    }
    Online::KeysRotate => {
      let kid = cloud.host_rotate_keys().await?;
      writeln!(
        io::stdout().lock(),
        "New signing key {kid}; prepare, sign and publish a key set next."
      )?;
      Ok(())
    }
    Online::KeysPrepare(path) => {
      let statement = cloud.host_prepare_key_set().await?;
      let json = serde_json::to_vec_pretty(&statement).map_err(|_| CloudError::StoredData)?;
      std::fs::write(&path, json)?;
      writeln!(
        io::stdout().lock(),
        "Key set statement written to {}; sign it offline.",
        path.display()
      )?;
      Ok(())
    }
    Online::KeysPublish(path) => {
      cloud.host_publish_key_set(&std::fs::read_to_string(&path)?).await?;
      writeln!(io::stdout().lock(), "Key set published.")?;
      Ok(())
    }
  }
}

fn arguments() -> Result<(Command, Option<PathBuf>), CloudError> {
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
  let args = args
    .into_iter()
    .map(|arg| arg.into_string().map_err(|_| CloudError::Configuration(USAGE)))
    .collect::<Result<Vec<_>, _>>()?;
  let words: Vec<&str> = args.iter().map(String::as_str).collect();
  let path = |value: &str| Path::new(value).to_path_buf();
  let command = match words.as_slice() {
    [] | ["serve"] => Command::Online(Online::Serve),
    ["root", "generate", root] => Command::Offline(Offline::RootGenerate(path(root))),
    ["keys", "rotate"] => Command::Online(Online::KeysRotate),
    ["keys", "prepare", statement] => Command::Online(Online::KeysPrepare(path(statement))),
    ["keys", "sign", statement, root, signed] => Command::Offline(Offline::KeysSign {
      statement: path(statement),
      root: path(root),
      signed: path(signed),
    }),
    ["keys", "publish", signed] => Command::Online(Online::KeysPublish(path(signed))),
    _ => return Err(CloudError::Configuration(USAGE)),
  };
  Ok((command, config))
}
