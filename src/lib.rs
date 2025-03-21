#![warn(clippy::nursery, clippy::pedantic)]
#![allow(clippy::module_name_repetitions, reason = "Too much nagging")]
#![allow(
    clippy::future_not_send,
    reason = "bimap's Iter type is (probably) incorrectly not marked Send + Sync which in turn affects the futures"
)]
#![allow(clippy::missing_const_for_fn, reason = "Too much nagging and many false positives")]

mod commands;
mod config;
mod helper;
mod local_fs;
mod remote_fs;
mod tag_repository;
mod updater;

use helper::{IntoOk, newtype, take_last_n_chars};
pub use tag_repository::SyncedPath;
pub use helper::SyncedPathPrinter;

pub use commands::*;
pub use config::{Config, load_config};
pub use local_fs::{
    FileError, FileSystemLoopError, LocalError, LocalFs, LocalFsWalker, get_tags_of_file,
};
pub use remote_fs::{
    Body, Connection, CreateTag, DeserializeError, FileId, FileMap, ListFilesWithTag, ListTags,
    ListTagsError, ListTagsMultiStatus, Parse, RemoteFs, Request, TagFile, TagId, TagMap,
    UntagFile, parse,
};
pub use tag_repository::{FileLocation, PrefixMapping, Repository, Side, Tag, Tags};

pub use updater::{InitError, Initialized, Uninitialized, in_memory_patch};

#[expect(
    async_fn_in_trait,
    reason = "Implementations don't return Send+Sync futures anyway due to limitation in bimap"
)]
pub trait FileSystem {
    async fn create_repo(&mut self) -> Result<Repository, InitError>;
    async fn update_tags<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = Command> + Send;
}
