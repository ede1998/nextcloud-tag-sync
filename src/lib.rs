mod config;
mod helper;
mod local_fs;
mod remote_fs;
mod requests;
mod tag_repository;

use helper::{take_last_n_chars, IntoOk};

pub use config::{load_config, Config, ConfigError};
pub use helper::ErrorCollection;
pub use local_fs::{FileSystemLoopError, LocalFsWalker};
pub use remote_fs::{ListTagsError, RemoteFsWalker};
pub use requests::{Connection, ListFilesWithTag, ListTags};
pub use tag_repository::{PrefixMapping, Repository, Tags};
