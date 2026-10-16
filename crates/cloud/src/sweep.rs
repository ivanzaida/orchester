use std::time::Duration;

use tokio::task::JoinHandle;

use crate::{Cloud, clock::DAY_MS, db::time, error::CloudError};

pub(crate) fn spawn(cloud: Cloud) -> JoinHandle<()> {
  tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(60));
    loop {
      interval.tick().await;
      if let Err(error) = cloud.sweep().await {
        error.log();
      }
    }
  })
}

impl Cloud {
  /// Deletes what the cloud no longer needs: expired links and invites, accounts never confirmed whose links all
  /// expired, and the deleted accounts' tokens past the time they would have ended unused.
  /// Runs every minute while the cloud serves.
  pub async fn sweep(&self) -> Result<(), CloudError> {
    let now = self.now();
    let client = self.inner.db.client().await?;
    client
      .execute(
        "DELETE FROM accounts a WHERE a.confirmed_at IS NULL AND a.created_at <= $2
         AND NOT EXISTS (SELECT 1 FROM email_links l WHERE l.account = a.id AND l.purpose = 'confirm' AND l.expires_at > $1)",
        &[&time(now), &time(now - DAY_MS)],
      )
      .await?;
    client
      .execute("DELETE FROM email_links WHERE expires_at <= $1", &[&time(now)])
      .await?;
    client
      .execute("DELETE FROM invites WHERE expires_at <= $1", &[&time(now)])
      .await?;
    client
      .execute("DELETE FROM revoked_invites WHERE until <= $1", &[&time(now)])
      .await?;
    client
      .execute("DELETE FROM used_link_statements WHERE expires_at <= $1", &[&time(now)])
      .await?;
    client
      .execute("DELETE FROM ended_tokens WHERE until <= $1", &[&time(now)])
      .await?;
    Ok(())
  }
}
