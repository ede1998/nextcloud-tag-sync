use std::collections::HashSet;

use bimap::BiMap;
use tracing::{debug, error, warn};

use crate::{
    Command, Connection, CreateTag, FileId, Modification, SyncedPath, Tag, TagFile, TagId,
    UntagFile,
};

use super::common::LimitedConcurrency;

pub struct RemoteFs {
    pub tags: BiMap<TagId, Tag>,
    pub files: BiMap<FileId, SyncedPath>,
}

impl RemoteFs {
    pub async fn update<I>(
        &mut self,
        commands: I,
        connection: &Connection,
        max_concurrent_requests: usize,
    ) where
        I: IntoIterator<Item = Command>,
        I::IntoIter: Clone,
    {
        let commands = commands.into_iter();
        self.create_missing_tags(commands.clone(), max_concurrent_requests, connection)
            .await;

        LimitedConcurrency::new(commands, max_concurrent_requests)
            .transform(|cmd| self.run_command(cmd, connection))
            .execute()
            .await;
    }

    async fn create_missing_tags<I>(
        &mut self,
        commands: I,
        max_concurrent_requests: usize,
        connection: &Connection,
    ) where
        I: IntoIterator<Item = Command>,
    {
        let tags_to_create = self.get_unknown_tags(commands);
        let new_tags = LimitedConcurrency::new(tags_to_create, max_concurrent_requests)
            .transform(
                |tag| async move { (tag.clone(), connection.request(CreateTag::new(tag)).await) },
            )
            .aggregate(
                |new_tags: &mut BiMap<TagId, Tag>, (tag, result)| match result {
                    Ok(tag_id) => {
                        new_tags.insert(tag_id, tag);
                    }
                    Err(e) => {
                        warn!("Failed to create tag {tag}: {e}");
                    }
                },
            )
            .collect_into()
            .await;
        self.tags.extend(new_tags);
    }

    fn get_unknown_tags<I>(&self, commands: I) -> HashSet<Tag>
    where
        I: IntoIterator<Item = Command>,
    {
        let is_unknown_tag = |tag: &Tag| -> bool { !self.tags.contains_right(tag) };
        commands
            .into_iter()
            .flat_map(|cmd| cmd.actions)
            .filter_map(|action| match action.modification {
                Modification::Add => is_unknown_tag(&action.tag).then_some(action.tag),
                Modification::Remove => {
                    if is_unknown_tag(&action.tag) {
                        warn!("Removed tag {} but it is not known?", action.tag);
                    }
                    None
                }
            })
            .collect()
    }

    async fn run_command(&self, cmd: Command, connection: &Connection) {
        let path = &cmd.path;

        let Some(&file_id) = self.files.get_by_right(path) else {
            error!("Unknown file {path}. Ensure file is synced so it has an ID.");
            return;
        };

        for action in cmd.actions {
            let tag = &action.tag;

            let Some(&tag_id) = self.tags.get_by_right(&action.tag) else {
                    // We created unknown tags before. Can only land here if tag creation failed.
                    error!("Unknown tag {tag}. Failed to update tags for file {path}.");
                    continue;
                };

            let res = match action.modification {
                Modification::Add => connection.request(TagFile::new(tag_id, file_id)).await,
                Modification::Remove => connection.request(UntagFile::new(tag_id, file_id)).await,
            };

            match res {
                Ok(_) => {
                    let updated = match action.modification {
                        Modification::Add => "added",
                        Modification::Remove => "removed",
                    };

                    debug!("Successfully {updated} tag {tag} for file {path}");
                }
                Err(e) => {
                    error!("Failed to update tag {tag} for file {path}: {e}",);
                }
            }
        }
    }
}
