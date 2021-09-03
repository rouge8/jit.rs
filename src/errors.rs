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
    #[error("There is no merge in progress ({0} missing).")]
    NoMergeInProgress(String),
    #[error("There is no merge to abort ({0} missing).")]
    NoMergeToAbort(String),
    #[error("pathspec '{0}' did not match any files")]
    RmUntrackedFile(String),
    #[error("not removing '{0}' recursively without -r")]
    RmNotRecursive(String),
    #[error("jit rm: '{0}': Operation not permitted")]
    RmOperationNotPermitted(String),
    #[error("There was a problem with the editor '{0}'")]
    ProblemWithEditor(String),
    #[error("You seem to have moved HEAD. Not rewinding, check your HEAD!")]
    UnsafeRewind,
    #[error("bad config line {0} in file {1}")]
    ConfigParseError(usize, PathBuf),
    #[error("cannot overwrite multiple values with a single value")]
    ConfigConflict,
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
