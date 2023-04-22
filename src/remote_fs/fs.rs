use bimap::BiHashMap;

use crate::{SyncedPath, Tag, TagId, FileId};

pub struct RemoteFs {
    pub tags: BiHashMap<TagId, Tag>,
    pub files: BiHashMap<FileId, SyncedPath>,
}
