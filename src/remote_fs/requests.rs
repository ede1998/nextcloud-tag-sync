mod common;
mod create_tag;
mod get_file_id;
mod list_files_with_tag;
mod list_tags;
mod tag_file;
mod untag_file;

use common::{empty_as_none, str_to_method};

pub use common::{Connection, RequestError};
pub use create_tag::CreateTag;
pub use get_file_id::GetFileId;
pub use list_files_with_tag::ListFilesWithTag;
pub use list_tags::ListTags;
pub use tag_file::TagFile;
pub use untag_file::UntagFile;
pub type ListTagsMultiStatus = list_tags::MultiStatus;

pub use common::{Body, DeserializeError, Parse, Request, parse};
