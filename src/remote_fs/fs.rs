use bimap::BiMap;

use crate::{FileId, SyncedPath, Tag, TagId};

pub struct RemoteFs {
    pub tags: BiMap<TagId, Tag>,
    pub files: BiMap<FileId, SyncedPath>,
}
