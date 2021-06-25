use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("{method}('{path}'): Permission denied")]
    NoPermission { method: String, path: PathBuf },
    #[error("Signature: expected '{expected}' but found '{got}'")]
    InvalidSignature { expected: String, got: String },
    #[error("Version: expected '{expected}' but found '{got}'")]
    InvalidVersion { expected: u32, got: u32 },
    #[error("Checksum does not match value stored on disk")]
    InvalidChecksum,
    #[error("Unable to create '{0}': File exists.")]
    LockDenied(PathBuf),
    #[error("{0}")]
    InvalidBranch(String),
    #[error("{0}")]
    InvalidObject(String),
    #[error("MigrationConflict")]
    MigrationConflict,
    #[error("branch '{0}' not found.")]
    BranchNotFound(String),
    #[error("'{0}' is not a jit command.")]
    UnknownCommand(String),
    #[error("Exit {0}")]
    Exit(i32),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<std::array::TryFromSliceError> for Error {
    fn from(err: std::array::TryFromSliceError) -> Error {
        Error::Other(format!("{}", err))
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Error {
        Error::Other(format!("{}", err))
    }
}
