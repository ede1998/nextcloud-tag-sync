use crate::{
    SyncedPath, SyncedPathPrinter, Tag, Tags,
    tag_repository::DiffResult,
};

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash, PartialOrd, Ord)]
pub enum Modification {
    Add,
    Remove,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub struct TagAction {
    pub tag: Tag,
    pub modification: Modification,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, PartialOrd, Ord)]
pub struct Command {
    pub path: SyncedPath,
    pub actions: Vec<TagAction>,
}

impl Command {
    const fn new(path: SyncedPath) -> Self {
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

    #[must_use]
    pub fn none_if_empty(self) -> Option<Self> {
        (!self.actions.is_empty()).then_some(self)
    }
}

pub fn resolve_diffs<I>(iter: I) -> Vec<Command>
where
    I: IntoIterator<Item = DiffResult>,
{
    iter.into_iter()
        .filter_map(|res| {
            Command::new(res.path)
                .remove(res.tags.left_only)
                .add(res.tags.right_only)
                .none_if_empty()
        })
        .collect()
}

pub struct CommandsFormatter<'a>(pub &'a [Command]);

impl std::fmt::Display for CommandsFormatter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }

        let printer: SyncedPathPrinter<_> = self
            .0
            .iter()
            .map(|cmd| (&cmd.path, ActionsFormatter(&cmd.actions)))
            .collect();
        write!(f, "{printer}")?;
        Ok(())
    }
}

#[derive(Default)]
pub struct ActionsFormatter<'a>(pub &'a [TagAction]);

impl std::fmt::Display for ActionsFormatter<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }

        f.write_str(" ->")?;
        for action in self.0 {
            let sign = match action.modification {
                Modification::Add => "+",
                Modification::Remove => "-",
            };
            write!(f, " {sign}{}", action.tag)?;
        }
        Ok(())
    }
}
