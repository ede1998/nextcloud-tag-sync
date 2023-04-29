use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;

use bimap::BiMap;
use snafu::prelude::*;
use tracing::debug;
use tracing::error;
use tracing::warn;

use super::{DeserializeError, RemoteFs, RequestError};
use crate::remote_fs::common::LimitedConcurrency;
use crate::FileId;
use crate::Tag;
use crate::{Connection, IntoOk, ListFilesWithTag, ListTags, PrefixMapping, Repository, Tags};

pub struct RemoteFsWalker<'a> {
    connection: Connection,
    prefixes: &'a [PrefixMapping],
    max_concurrent_requests: usize,
}

impl<'a> RemoteFsWalker<'a> {
    pub fn new(
        connection: Connection,
        prefixes: &'a [PrefixMapping],
        max_concurrent_requests: usize,
    ) -> Self {
        Self {
            connection,
            prefixes,
            max_concurrent_requests,
        }
    }

    pub async fn build_repository(&self) -> Result<(Repository, RemoteFs), ListTagsError> {
        let tag_map = self
            .connection
            .request(ListTags)
            .await
            .context(ListTagsSnafu)?;

        debug!("Received mapping of {} tags", tag_map.len());

        let file_tag_helper = LimitedConcurrency::new(&tag_map, self.max_concurrent_requests)
            .transform(|(id, tag)| async move {
                (
                    tag,
                    self.connection.request(ListFilesWithTag::new(*id)).await,
                )
            })
            .aggregate(
                |tags: &mut FileTagHelper, (tag, result): (&Tag, Result<Vec<_>, _>)| match result {
                    Ok(files) => {
                        debug!("Processing tag {tag} with {} files", files.len());
                        tags.group_tags_by_file(tag, files);
                    }
                    Err(err) => error!("Failed to fetch file for tag {tag}: {err}"),
                },
            )
            .collect_into()
            .await;

        let mut repo = Repository::new(self.prefixes.into());
        let mut files = BiMap::with_capacity(file_tag_helper.file_ids.len());
        for (file, tags) in file_tag_helper.file_tags {
            let synced_path = repo.insert_remote(Path::new(&file), tags);
            let Some(&id) = file_tag_helper.file_ids.get_by_right(&file) else {
                warn!("Missing id for file {file}");
                continue;
            };
            files.insert(id, synced_path);
        }

        let fs = RemoteFs {
            tags: tag_map,
            files,
        };

        Ok((repo, fs))
    }
}

#[derive(Debug, Snafu)]
#[snafu(display("Failed to list tags: {source}"))]
pub struct ListTagsError {
    pub source: RequestError<DeserializeError>,
}

#[derive(Debug, Default)]
struct FileTagHelper {
    file_ids: BiMap<FileId, String>,
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
                Entry::Occupied(mut entry) => entry.get_mut().insert_all(&tag),
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
