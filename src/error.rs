use std::fmt;

/// Every failure IFHF reports. The variant decides the process exit code.
#[derive(Debug)]
pub enum Error {
    Usage(String),
    NotFound(String),
    Ambiguous(String),
    Backend(String),
    Config(String),
    Metadata(String),
    NeedsConfirmation(String),
    Aborted,
    Partial(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// 0 ok, 1 failure, 2 usage, 3 not found, 4 ambiguous,
    /// 5 confirmation required (non-interactive without --yes), 6 cancelled by the user.
    pub fn exit_code(&self) -> i32 {
        match self {
            Error::Usage(_) => 2,
            Error::NotFound(_) => 3,
            Error::Ambiguous(_) => 4,
            Error::NeedsConfirmation(_) => 5,
            Error::Aborted => 6,
            _ => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m)
            | Error::NotFound(m)
            | Error::Ambiguous(m)
            | Error::Backend(m)
            | Error::Config(m)
            | Error::Metadata(m)
            | Error::NeedsConfirmation(m)
            | Error::Partial(m) => f.write_str(m),
            Error::Aborted => f.write_str("Cancelled."),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Backend(e.to_string())
    }
}
