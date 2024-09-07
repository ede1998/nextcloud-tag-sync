mod fs;
mod fs_walker;

use fs::{get_tags_of_file, FileError};

pub use fs::{LocalFs, LocalError};
pub use fs_walker::{FileSystemLoopError, LocalFsWalker};
