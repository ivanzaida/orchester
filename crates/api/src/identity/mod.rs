//! Orchester Cloud's identity tokens and signed key sets (`docs/architecture/server.md`, "Identity tokens and keys"):
//! the cloud signs, team servers verify locally with the key set its offline root key signed.
//!
//! Both are compact JWS (`EdDSA`, Ed25519). A team server keeps the newest key set that verifies with the root it
//! pinned, and accepts a token whose key that set lists. Remembering each token's `jti` until its expiry, so a token
//! is accepted once, is the team server's job.

mod error;
mod jws;
mod key_set;
mod keys;
mod token;

pub use error::IdentityError;
pub use key_set::{KEY_SET_LIFETIME_SECS, KeySetStatement, ListedKey, VerifiedKeySet};
pub use keys::{PrivateKey, PublicKey, RootKey, SigningKey};
pub use token::{IdentityClaims, TOKEN_LIFETIME_SECS, TokenSubject};

/// The clock difference between the cloud and a team server that verification tolerates.
pub const CLOCK_SKEW_SECS: i64 = 2 * 60;
