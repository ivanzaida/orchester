use thiserror::Error;

/// Why a token or key set was refused. No variant carries the token, a key or a claim value.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
  #[error("malformed token or key set")]
  Malformed,
  #[error("unsupported signature algorithm")]
  UnsupportedAlgorithm,
  #[error("signature does not verify")]
  BadSignature,
  #[error("the key set does not list the token's key")]
  UnknownKey,
  #[error("the token's key could not sign at the token's time")]
  KeyNotValidAtTime,
  #[error("the token was issued by another cloud")]
  WrongIssuer,
  #[error("the token is for another team server")]
  WrongAudience,
  #[error("the token has expired")]
  Expired,
  #[error("the token or key set is not valid yet")]
  NotYetValid,
  #[error("the key set has expired")]
  KeySetExpired,
  #[error("invalid key")]
  InvalidKey,
  #[error("the key set statement is inconsistent or outlives the rules")]
  InvalidStatement,
  #[error("the key set statement was not prepared within the last day")]
  StaleStatement,
  #[error("cannot generate random data")]
  Random,
}
