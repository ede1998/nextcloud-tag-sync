use crate::{
    tag_repository::{DiffResult, Side},
    SyncedPath, Tag, Tags,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Modification {
    Add,
    Remove,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TagAction {
    pub tag: Tag,
    pub modification: Modification,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Command {
    pub path: SyncedPath,
    pub actions: Vec<TagAction>,
}

impl Command {
    fn new(path: SyncedPath) -> Self {
        Self {
            path,
            actions: Vec::new(),
        }
    }

    fn add(mut self, tags: Tags) -> Self {
        self.actions.extend(tags.into_iter().map(|tag| TagAction {
            modification: Modification::Add,
            tag,
        }));
        self
    }

    fn remove(mut self, tags: Tags) -> Self {
        self.actions.extend(tags.into_iter().map(|tag| TagAction {
            modification: Modification::Remove,
            tag,
        }));
        self
    }

    pub fn none_if_empty(self) -> Option<Self> {
        (!self.actions.is_empty()).then_some(self)
    }
}

pub fn resolve_diffs<I>(iter: I, source_of_truth: Side) -> (Vec<Command>, Vec<Command>)
where
    I: IntoIterator<Item = DiffResult>,
{
    match source_of_truth {
        Side::Left => {
            let left = iter
                .into_iter()
                .filter_map(|res| {
                    Command::new(res.path)
                        .add(res.left_only)
                        .remove(res.right_only)
                        .none_if_empty()
                })
                .collect();
            (left, Vec::new())
        }
        Side::Right => {
            let right = iter
                .into_iter()
                .filter_map(|res| {
                    Command::new(res.path)
                        .remove(res.left_only)
                        .add(res.right_only)
                        .none_if_empty()
                })
                .collect();
            (Vec::new(), right)
        }
        Side::Both => {
            let mut right = Vec::new();
            let mut left = Vec::new();

            for res in iter {
                push_some(
                    &mut right,
                    Command::new(res.path.clone())
                        .add(res.left_only)
                        .none_if_empty(),
                );
                push_some(
                    &mut left,
                    Command::new(res.path).add(res.right_only).none_if_empty(),
                );
            }

            (left, right)
        }
    }
}

fn push_some<T>(vec: &mut Vec<T>, item: Option<T>) {
    if let Some(t) = item {
        vec.push(t);
    }
}
