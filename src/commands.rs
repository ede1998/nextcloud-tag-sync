use crate::{
    tag_repository::{DiffResult, Side},
    SyncedPath, Tag, Tags,
};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Action {
    Add(Tag),
    Remove(Tag),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Command {
    pub path: SyncedPath,
    pub actions: Vec<Action>,
}

impl Command {
    fn new(path: SyncedPath) -> Self {
        Self {
            path,
            actions: Vec::new(),
        }
    }

    fn add(mut self, tags: Tags) -> Self {
        self.actions.extend(tags.into_iter().map(Action::Add));
        self
    }

    fn remove(mut self, tags: Tags) -> Self {
        self.actions.extend(tags.into_iter().map(Action::Remove));
        self
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
                .map(|res| {
                    Command::new(res.path)
                        .add(res.left_only)
                        .remove(res.right_only)
                })
                .collect();
            (left, Vec::new())
        }
        Side::Right => {
            let right = iter
                .into_iter()
                .map(|res| {
                    Command::new(res.path)
                        .remove(res.left_only)
                        .add(res.right_only)
                })
                .collect();
            (Vec::new(), right)
        }
        Side::Both => {
            let mut right = Vec::new();
            let mut left = Vec::new();

            for res in iter {
                right.push(Command::new(res.path.clone()).add(res.left_only));
                left.push(Command::new(res.path).add(res.right_only));
            }

            (left, right)
        }
    }
}
