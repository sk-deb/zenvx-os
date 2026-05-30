//! Shared types, constants and errors for the ZenvX OS userspace.

pub mod config;

/// Product name shown everywhere (boot screen, --version, UI).
pub const NAME: &str = "ZenvX OS";
/// Workspace version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Top-level error type shared across crates.
#[derive(Debug)]
pub enum Error {
    /// A subsystem failed with a human-readable message.
    Msg(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Msg(m) => write!(f, "{m}"),
        }
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    #[test]
    fn brand_is_stable() {
        assert_eq!(super::NAME, "ZenvX OS");
    }
}
