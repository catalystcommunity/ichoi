//! Hand-rolled error type (house convention for pure library crates: no derive macros).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// An account id was not of the form `uuid@domain.tld`.
    InvalidAccountId(String),
    /// A client-satellite name suffix was empty or too long.
    InvalidSuffix(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainError::InvalidAccountId(s) => {
                write!(f, "invalid account id (expected uuid@domain): {s}")
            }
            DomainError::InvalidSuffix(s) => write!(f, "invalid share suffix: {s}"),
        }
    }
}

impl std::error::Error for DomainError {}
