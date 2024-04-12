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
    TagFile, TagId, UntagFile,
};
pub use tag_repository::{PrefixMapping, Repository, Tag, Tags};

use local_fs::execute as execute_locally;

pub use updater::{InitError, Initialized, LocalError, Uninitialized};

trait FileSystem {
    async fn create_repo(&mut self) -> Result<Repository, InitError>;
    async fn update_tags<I: IntoIterator<Item = Command>>(&mut self, commands: I);
}
