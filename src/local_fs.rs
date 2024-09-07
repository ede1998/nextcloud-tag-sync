mod fs;
mod fs_walker;

use fs::{get_tags_of_file, FileError};

pub use fs::{LocalError, LocalFs};
pub use fs_walker::{FileSystemLoopError, LocalFsWalker};
