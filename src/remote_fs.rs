use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;

use futures::StreamExt;
use snafu::prelude::*;
use snafu::Whatever;
use tracing::debug;
use tracing::error;

use crate::Tags;
use crate::{Connection, IntoOk, ListFilesWithTag, ListTags, PrefixMapping, Repository};

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

    pub async fn build_repository(&self) -> Result<Repository, Whatever> {
        let tag_map = self
            .connection
            .request(ListTags)
            .await
            .whatever_context("failed to list tags")?;

        debug!("Received mapping of {} tags", tag_map.len());

        let tags = futures::stream::iter(tag_map)
            .map(|(id, tag)| async move {
                (
                    tag,
                    self.connection.request(ListFilesWithTag::new(id)).await,
                )
            })
            .buffer_unordered(self.max_concurrent_requests)
            .fold(FileToTags::default(), |mut tags, (tag, result)| {
                match result {
                    Ok(files) => {
                        debug!("Processing tag {tag} with {} files", files.len());
                        tags.group_tags_by_file(&tag, files);
                    }
                    Err(err) => error!("Failed to fetch file for tag {tag}: {err}"),
                }
                futures::future::ready(tags)
            })
            .await;

        let mut repo = Repository::new(self.prefixes.into());
        for (file, tags) in tags {
            repo.insert_remote(Path::new(&file), tags);
        }
        Ok(repo)
    }
}

#[derive(Debug, Default)]
struct FileToTags(HashMap<String, Tags>);

impl FileToTags {
    fn group_tags_by_file<I: IntoIterator<Item = String>>(&mut self, tag: &str, files: I) {
        #[allow(unstable_name_collisions)]
        let tag: Tags = tag.parse().into_ok();
        for file in files {
            match self.0.entry(file) {
                Entry::Occupied(mut entry) => entry.get_mut().insert_all(&tag),
                Entry::Vacant(entry) => {
                    entry.insert(tag.clone());
                }
            }
        }
    }
}

impl IntoIterator for FileToTags {
    type Item = (String, Tags);

    type IntoIter = std::collections::hash_map::IntoIter<String, Tags>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_tags() {
        let files = (0..2000).map(|i| format!("/basic/{i}/bla"));
        let files1 = (0..2000).map(|i| format!("/basic/{i}/blub"));
        let mut ftt = FileToTags::default();
        ftt.group_tags_by_file("tag", files.clone());
        ftt.group_tags_by_file("tag1", files.clone());
        ftt.group_tags_by_file("tag2", files.clone());
        ftt.group_tags_by_file("tag3", files);
        ftt.group_tags_by_file("tag3", files1);
        let res: HashMap<_,_> = ftt.into_iter().collect();
        assert_eq!(res.len(), 4000);
        for tags in res.values() {
            assert!(tags.len() <= 4);
        }
    }
}
