use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    path::Path,
    sync::Arc,
};

use snafu::{ResultExt, Snafu};
use tracing::{debug, error, warn};

use crate::{
    Command, Config, Connection, CreateTag, FileId, FileSystem, IntoOk, Modification, SyncedPath,
    Tag, TagFile, TagId, Tags, UntagFile, updater::RemoteSnafu,
};

use super::{DeserializeError, GetFileId, RequestError, common::LimitedConcurrency};

pub type FileMap = bimap::BiHashMap<FileId, SyncedPath>;
pub type TagMap = bimap::BiHashMap<TagId, Tag>;

#[derive(Debug)]
pub struct RemoteFs {
    tags: TagMap,
    files: FileMap,
    config: Arc<Config>,
}

impl RemoteFs {
    #[must_use]
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            tags: TagMap::default(),
            files: FileMap::default(),
            config,
        }
    }

    async fn create_missing_tags<I>(&mut self, commands: I, connection: &Connection)
    where
        I: IntoIterator<Item = Command> + Send,
    {
        let tags_to_create = self.get_unknown_tags(commands);
        let new_tags = LimitedConcurrency::new(tags_to_create, self.config.max_concurrent_requests)
            .transform(
                |tag| async move { (tag.clone(), connection.request(CreateTag::new(tag)).await) },
            )
            .aggregate(|new_tags: &mut TagMap, (tag, result)| match result {
                Ok(tag_id) => {
                    new_tags.insert(tag_id, tag);
                }
                Err(e) => {
                    warn!("Failed to create tag {tag}: {e}");
                }
            })
            .collect_into()
            .await;
        self.tags.extend(new_tags);
    }

    async fn load_tags(&mut self, connection: &Connection) -> Result<(), ListTagsError> {
        let tag_map = connection
            .request(crate::ListTags)
            .await
            .context(ListTagsSnafu)?;
        debug!("Received mapping of {} tags", tag_map.len());
        self.tags.extend(tag_map);

        Ok(())
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

    async fn get_missing_file_ids<I>(&mut self, commands: I, connection: &Connection)
    where
        I: IntoIterator<Item = Command> + Send,
        I::IntoIter: Send,
    {
        let Config {
            max_concurrent_requests,
            prefixes,
            ..
        } = &*self.config;
        let missing_file_id_requests = commands
            .into_iter()
            .map(|cmd| cmd.path)
            .filter(|path| !self.files.contains_right(path))
            .filter_map(|path| {
                let request = GetFileId::new(&path.remote_file(prefixes));

                if request.is_none() {
                    warn!("failed to format file {path} as UTF-8");
                }

                request.map(|req| (path, req))
            });

        let new_files = LimitedConcurrency::new(missing_file_id_requests, *max_concurrent_requests)
            .transform(|(path, request)| async move { (path, connection.request(request).await) })
            .aggregate(|new_files: &mut FileMap, (path, result)| match result {
                Ok(file_id) => {
                    new_files.insert(file_id, path);
                }
                Err(e) => {
                    warn!("failed to query file id for {path}: {e}");
                }
            })
            .collect_into()
            .await;
        self.files.extend(new_files);
    }

    async fn run_command(&self, cmd: Command, connection: &Connection) {
        let path = &cmd.path;

        let Some(&file_id) = self.files.get_by_right(path) else {
            // We queried unknown file ids before. Can only land here if query failed.
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
                Ok(()) => {
                    let updated = match action.modification {
                        Modification::Add => "added",
                        Modification::Remove => "removed",
                    };

                    debug!("Successfully {updated} tag {tag} for file {path}");
                }
                Err(e) => {
                    // TODO handle this case for remote and also local fs
                    // What happens if update fails: cached repo should not be updated
                    // for this file tag but it will be right now. This will lead to
                    // issues in the next reverse direction run with tags being reset to the previous
                    // state.
                    // This can especially happen when a directory is tagged in Nextcloud as at least
                    // BTRFS does not support tagging directories.
                    error!("Failed to update tag {tag} for file {path}: {e}",);
                }
            }
        }
    }
}

impl FileSystem for RemoteFs {
    async fn create_repo(&mut self) -> Result<crate::Repository, crate::InitError> {
        use crate::{ListFilesWithTag, Repository};
        let connection = &Connection::from_config(&self.config);
        self.load_tags(connection).await.context(RemoteSnafu)?;
        let file_tag_helper =
            LimitedConcurrency::new(&self.tags, self.config.max_concurrent_requests)
                .transform(|(id, tag)| async move {
                    (tag, connection.request(ListFilesWithTag::new(*id)).await)
                })
                .aggregate(
                    |tags: &mut FileTagHelper, (tag, result): (&Tag, Result<Vec<_>, _>)| {
                        match result {
                            Ok(files) => {
                                debug!("Processing tag {tag} with {} files", files.len());
                                tags.group_tags_by_file(tag, files);
                            }
                            Err(err) => error!("Failed to fetch file for tag {tag}: {err}"),
                        }
                    },
                )
                .collect_into()
                .await;
        let mut repo = Repository::new(self.config.prefixes.clone());
        for (file, tags) in file_tag_helper.file_tags {
            let Ok(synced_path) = repo
                .insert_remote(Path::new(&file), tags)
                .inspect_err(|e| tracing::debug!("Ignoring: {e}"))
            else {
                continue;
            };
            let Some(&id) = file_tag_helper.file_ids.get_by_right(&file) else {
                warn!("Missing id for file {file}");
                continue;
            };
            self.files.insert(id, synced_path);
        }

        tracing::info!("Finished building remote repo. {}", repo.stats());

        Ok(repo)
    }

    async fn update_tags<I>(&mut self, commands: I)
    where
        I: IntoIterator<Item = Command> + Send,
    {
        let connection = Connection::from_config(&self.config);
        let commands: Vec<_> = commands.into_iter().collect();
        if let Err(e) = self.load_tags(&connection).await {
            tracing::warn!("Failed to load existing tags from Nextcloud: {e}");
        }
        self.create_missing_tags(commands.clone(), &connection)
            .await;

        self.get_missing_file_ids(commands.clone(), &connection)
            .await;

        LimitedConcurrency::new(commands, self.config.max_concurrent_requests)
            .transform(|cmd| self.run_command(cmd, &connection))
            .execute()
            .await;
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("Failed to list tags: {source}"))]
pub struct ListTagsError {
    pub source: RequestError<DeserializeError>,
}

#[derive(Debug, Default)]
struct FileTagHelper {
    file_ids: bimap::BiHashMap<FileId, String>,
    file_tags: HashMap<String, Tags>,
}

impl FileTagHelper {
    fn group_tags_by_file<I: IntoIterator<Item = (FileId, String)>>(
        &mut self,
        tag: &str,
        files: I,
    ) {
        #[allow(unstable_name_collisions)]
        let tag: Tags = tag.parse().into_ok();
        for (id, file) in files {
            self.file_ids.insert(id, file.clone());
            match self.file_tags.entry(file) {
                Entry::Occupied(mut entry) => entry.get_mut().insert_all(tag.clone()),
                Entry::Vacant(entry) => {
                    entry.insert(tag.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_tags() {
        let files = (0..2000).map(|i| (FileId::from(i), format!("/basic/{i}/bla")));
        let files1 = (2000..4000).map(|i| (FileId::from(i), format!("/basic/{i}/blob")));
        let mut ftt = FileTagHelper::default();

        ftt.group_tags_by_file("tag", files.clone());
        ftt.group_tags_by_file("tag1", files.clone());
        ftt.group_tags_by_file("tag2", files.clone());
        ftt.group_tags_by_file("tag3", files);
        ftt.group_tags_by_file("tag3", files1);

        assert_eq!(ftt.file_tags.len(), 4000);
        for tags in ftt.file_tags.values() {
            assert!(tags.len() <= 4);
        }

        assert_eq!(ftt.file_ids.len(), 4000);
        for (id, file) in ftt.file_ids {
            assert!(file.contains(&id.to_string()));
        }
    }
}
