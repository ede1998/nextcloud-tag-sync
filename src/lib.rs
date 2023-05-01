mod commands;
mod config;
mod helper;
mod local_fs;
mod remote_fs;
mod tag_repository;
mod updater;

use helper::{newtype, take_last_n_chars, IntoOk, SyncedPathPrinter};
use tag_repository::SyncedPath;

pub use commands::*;
pub use config::{load_config, Config};
pub use local_fs::{FileSystemLoopError, LocalFsWalker};
pub use remote_fs::{
    Connection, CreateTag, FileId, ListFilesWithTag, ListTags, ListTagsError, RemoteFs,
    RemoteFsWalker, TagFile, TagId, UntagFile,
};
pub use tag_repository::{PrefixMapping, Repository, Tag, Tags};

use local_fs::execute as execute_locally;
use remote_fs::execute as execute_remotely;

pub use updater::{InitError, Initialized, LocalError, Uninitialized};
