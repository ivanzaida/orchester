//! `orchester-release`: CI's release tool (`docs/stable-build.md`, "Releases from CI").
//!
//! - `orchester-release publish --api <url> --version <version> --platform <windows|macos> --arch <x86_64|aarch64>
//!   --notes-file <file> <release file>...` signs each file with the key in `ORCHESTER_RELEASE_SIGNING_KEY` and uploads
//!   them with the release token in `ORCHESTER_CLOUD_RELEASE_TOKEN`; it fails unless the cloud accepts every file.
//! - `orchester-release key-id` names the public key and key id of the key in `ORCHESTER_RELEASE_SIGNING_KEY`.
//! - `orchester-release generate-key` makes a new key and shows its seed once, for the secret store; run it on your own
//!   computer, never in CI.
//!
//! The key and the token are read from the environment only, never from the command line, and never printed.

#![allow(
  clippy::print_stdout,
  clippy::print_stderr,
  reason = "a command-line tool reports on its output"
)]

use std::{env, fs, path::PathBuf, process::ExitCode};

use orchester_api::{
  client::{CloudAddress, CloudClient, ServerAddress},
  releases::{NewRelease, ReleaseArch, ReleasePlatform, ReleaseSigningKey, ReleaseVersion},
};
use orchester_core::secret::SecretValue;
use orchester_release::{Draft, ReleaseFile, create_draft, finalize, upload_file};

const SIGNING_KEY: &str = "ORCHESTER_RELEASE_SIGNING_KEY";
const RELEASE_TOKEN: &str = "ORCHESTER_CLOUD_RELEASE_TOKEN";

fn main() -> ExitCode {
  let arguments: Vec<String> = env::args().skip(1).collect();
  let outcome = match arguments.split_first() {
    Some((command, rest)) if command == "publish" => PublishArguments::parse(rest).and_then(publish),
    Some((command, [])) if command == "key-id" => key_id(),
    Some((command, [])) if command == "generate-key" => generate_key(),
    _ => Err("usage: orchester-release publish|key-id|generate-key (see docs/stable-build.md)".to_owned()),
  };
  match outcome {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("error: {error}");
      ExitCode::FAILURE
    }
  }
}

struct PublishArguments {
  api: String,
  version: ReleaseVersion,
  platform: ReleasePlatform,
  arch: ReleaseArch,
  notes: PathBuf,
  files: Vec<PathBuf>,
}

impl PublishArguments {
  fn parse(arguments: &[String]) -> Result<Self, String> {
    let (mut api, mut version, mut platform, mut arch, mut notes) = (None, None, None, None, None);
    let mut files = Vec::new();
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
      let mut value = || {
        arguments
          .next()
          .cloned()
          .ok_or_else(|| format!("{argument} needs a value"))
      };
      match argument.as_str() {
        "--api" => api = Some(value()?),
        "--version" => version = Some(ReleaseVersion::parse(&value()?).map_err(|error| error.to_string())?),
        "--platform" => platform = Some(ReleasePlatform::parse(&value()?).ok_or("--platform is windows or macos")?),
        "--arch" => arch = Some(ReleaseArch::parse(&value()?).ok_or("--arch is x86_64 or aarch64")?),
        "--notes-file" => notes = Some(PathBuf::from(value()?)),
        flag if flag.starts_with("--") => return Err(format!("unknown option {flag}")),
        file => files.push(PathBuf::from(file)),
      }
    }
    if files.is_empty() {
      return Err("publish needs at least one release file".to_owned());
    }
    Ok(Self {
      api: api.ok_or("publish needs --api")?,
      version: version.ok_or("publish needs --version")?,
      platform: platform.ok_or("publish needs --platform")?,
      arch: arch.ok_or("publish needs --arch")?,
      notes: notes.ok_or("publish needs --notes-file")?,
      files,
    })
  }
}

/// The key in `ORCHESTER_RELEASE_SIGNING_KEY`; whitespace around it, as a pasted secret may have, is dropped.
fn signing_key() -> Result<ReleaseSigningKey, String> {
  let seed =
    SecretValue::new(env::var(SIGNING_KEY).unwrap_or_default()).map_err(|_| format!("{SIGNING_KEY} is not set"))?;
  ReleaseSigningKey::parse(seed.expose().trim()).map_err(|error| format!("{SIGNING_KEY}: {error}"))
}

fn release_token() -> Result<SecretValue, String> {
  let token = env::var(RELEASE_TOKEN).unwrap_or_default().trim().to_owned();
  SecretValue::new(token).map_err(|_| format!("{RELEASE_TOKEN} is not set"))
}

fn key_id() -> Result<(), String> {
  let key = signing_key()?;
  println!("public key {}", key.public_key().to_hex());
  println!("key id {}", key.id());
  Ok(())
}

fn generate_key() -> Result<(), String> {
  let (key, seed) = ReleaseSigningKey::generate().map_err(|error| error.to_string())?;
  println!(
    "private key (the secret {SIGNING_KEY}; keep it nowhere else) {}",
    seed.expose()
  );
  println!("public key {}", key.public_key().to_hex());
  println!("key id {}", key.id());
  Ok(())
}

fn publish(arguments: PublishArguments) -> Result<(), String> {
  let key = signing_key()?;
  let token = release_token()?;
  let notes = fs::read_to_string(&arguments.notes)
    .map_err(|error| format!("cannot read {}: {error}", arguments.notes.display()))?;
  let api = ServerAddress::parse(&arguments.api).map_err(|error| format!("--api: {error}"))?;
  // Only API calls are made; the web address is the API's, unused.
  let client = CloudClient::new(CloudAddress::new(api.clone(), Some(api))).map_err(|error| error.to_string())?;
  println!("signing with the release key {}", key.id());
  let files = arguments
    .files
    .iter()
    .map(|path| ReleaseFile::read(path, &arguments.version, arguments.platform, arguments.arch, &key))
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?;
  // The deployed cloud refuses `ci_run` until it reads this contract (ORC-575); CI names its run after that.
  let release = NewRelease {
    version: arguments.version,
    notes,
    critical: false,
    ci_run: None,
  };
  let runtime = tokio::runtime::Builder::new_current_thread()
    .enable_all()
    .build()
    .map_err(|error| format!("cannot start the async runtime: {error}"))?;
  runtime
    .block_on(upload(&client, &token, &release, &files))
    .map_err(|error| error.to_string())
}

async fn upload(
  client: &CloudClient,
  token: &SecretValue,
  release: &NewRelease,
  files: &[ReleaseFile],
) -> Result<(), orchester_release::PublishError> {
  match create_draft(client, token, release).await? {
    Draft::Created => println!("created the draft {}", release.version),
    Draft::Existing => println!("the release {} exists already; taking up its draft", release.version),
  }
  for file in files {
    let declared = file.declared();
    let sent = upload_file(client, token, &release.version, file).await?;
    println!(
      "uploaded {} ({} bytes, sha256 {}, {sent} parts sent)",
      declared.name,
      declared.size,
      declared.sha256.to_hex()
    );
  }
  finalize(client, token, &release.version, files).await?;
  println!(
    "the cloud accepted every file; {} is published on Beta",
    release.version
  );
  Ok(())
}
