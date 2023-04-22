use bimap::BiMap;

use crate::{SyncedPath, Tag, TagId, FileId};

pub struct RemoteFs {
    pub tags: BiMap<TagId, Tag>,
    pub files: BiMap<FileId, SyncedPath>,
}
