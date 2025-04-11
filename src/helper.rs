use crate::tag_repository::SyncedPath;
use std::{
    cmp::Ordering,
    ffi::OsStr,
    fmt::{Display, Formatter},
};
use termtree::Tree;

pub trait IntoOk {
    fn into_ok(self) -> Self::T;
    type T;
}

impl<T> IntoOk for Result<T, std::convert::Infallible> {
    type T = T;
    fn into_ok(self) -> T {
        match self {
            Ok(o) => o,
        }
    }
}

fn into_either<T>(res: Result<T, T>) -> (bool, T) {
    match res {
        Ok(o) => (true, o),
        Err(o) => (false, o),
    }
}

pub fn take_last_n_chars(string: &str, n: usize) -> &str {
    let len = string
        .char_indices()
        .rev()
        .nth(n - 1)
        .map_or(0, |(idx, _)| idx);
    // Safety: we just computed the index via `char_indices`.
    // The fallback 0 is always valid even if the string is empty.
    unsafe { string.get_unchecked(len..) }
}

macro_rules! newtype {
    ($name:ident, $type_name:ident) => {
        #[derive(
            Debug,
            Copy,
            Clone,
            Eq,
            PartialOrd,
            Ord,
            PartialEq,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct $name($type_name);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        impl $name {
            #[must_use]
            pub const fn into_inner(self) -> $type_name {
                self.0
            }
        }

        impl From<$type_name> for $name {
            fn from(value: $type_name) -> Self {
                Self(value)
            }
        }

        impl std::str::FromStr for $name {
            type Err = <$type_name as std::str::FromStr>::Err;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse().map(Self)
            }
        }
    };
}

pub(crate) use newtype;

enum Item<'a, T>
where
    T: Display,
{
    Number(usize),
    String { name: &'a OsStr, extras: Option<T> },
}

impl<'a, T> Item<'a, T>
where
    T: Display,
{
    fn string(str: &'a str) -> Self {
        Self::String {
            name: OsStr::new(str),
            extras: None,
        }
    }

    const fn os_str(str: &'a OsStr) -> Self {
        Self::String {
            name: str,
            extras: None,
        }
    }
}

impl<T> Item<'_, T>
where
    T: Display,
{
    fn set_extras(&mut self, data: T) {
        if let Self::String { extras, .. } = self {
            *extras = Some(data);
        }
    }
}

impl<T> PartialEq for Item<'_, T>
where
    T: Display,
{
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Item::Number(l), Item::Number(r)) => l == r,
            (Item::String { name: l, .. }, Item::String { name: r, .. }) => l == r,
            (Item::Number(_), Item::String { .. }) | (Item::String { .. }, Item::Number(_)) => {
                false
            }
        }
    }
}

impl<T> PartialOrd for Item<'_, T>
where
    T: Display,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Item::Number(l), Item::Number(r)) => l.partial_cmp(r),
            (Item::String { name: l, .. }, Item::String { name: r, .. }) => l.partial_cmp(r),
            (Item::Number(_), Item::String { .. }) | (Item::String { .. }, Item::Number(_)) => None,
        }
    }
}

impl<T> Display for Item<'_, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            Item::Number(num) => write!(f, "{num}"),
            Item::String {
                name,
                extras: Some(extras),
            } => write!(f, "{}{}", name.to_string_lossy(), extras),
            Item::String { name, extras: None } => write!(f, "{}", name.to_string_lossy()),
        }
    }
}

pub struct SyncedPathPrinter<'a, T: Display> {
    tree: Tree<Item<'a, T>>,
}

impl<T> Display for SyncedPathPrinter<'_, T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{}", self.tree)
    }
}

impl<'a> FromIterator<&'a SyncedPath> for SyncedPathPrinter<'a, DisplayUnit> {
    fn from_iter<I>(collection: I) -> Self
    where
        I: IntoIterator<Item = &'a SyncedPath>,
    {
        collection
            .into_iter()
            .map(|x| (x, DisplayUnit))
            .collect::<SyncedPathPrinter<_>>()
    }
}

impl<'a, T> FromIterator<(&'a SyncedPath, T)> for SyncedPathPrinter<'a, T>
where
    T: Display,
{
    fn from_iter<I>(collection: I) -> Self
    where
        I: IntoIterator<Item = (&'a SyncedPath, T)>,
    {
        use std::path::Component;

        let mut paths: Vec<_> = collection.into_iter().collect();
        paths.sort_unstable_by(|l, r| l.0.cmp(r.0));

        let mut tree = Tree::new(Item::string("ROOT"));
        for (path, extras) in paths {
            let root_id = path.root().into_inner();
            let mut tree = if let Some(t) = tree.leaves.get_mut(root_id) {
                t
            } else {
                tree.leaves
                    .extend((tree.leaves.len()..=root_id).map(|id| Tree::new(Item::Number(id))));
                &mut tree.leaves[root_id]
            };

            let components: &mut dyn Iterator<Item = Component> =
                if path.relative().as_os_str().is_empty() {
                    &mut std::iter::once(Component::Normal(OsStr::new("")))
                } else {
                    &mut path.relative().components()
                };

            for component in components {
                let element = Item::os_str(component.as_os_str());
                let (already_exists, element_at) = into_either(tree.leaves.binary_search_by(|x| {
                    x.root
                        .partial_cmp(&element)
                        .expect("should only compare strings here")
                }));
                if !already_exists {
                    tree.leaves.insert(element_at, Tree::new(element));
                }
                tree = &mut tree.leaves[element_at];
            }

            tree.root.set_extras(extras);
        }
        SyncedPathPrinter { tree }
    }
}

#[derive(Default)]
struct DisplayUnit;

impl Display for DisplayUnit {
    fn fmt(&self, _: &mut Formatter) -> std::fmt::Result {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> [SyncedPath; 15] {
        [
            SyncedPath::new(1, "test/house.txt"),
            SyncedPath::new(0, "bar/baz/random.txt"),
            SyncedPath::new(1, "test/mouse.txt"),
            SyncedPath::new(1, "test/country/word.txt"),
            SyncedPath::new(0, "dummy/err.pdf"),
            SyncedPath::new(1, "test/hello/tree.txt"),
            SyncedPath::new(0, "asdf.xml"),
            SyncedPath::new(1, "test/hello/world.txt"),
            SyncedPath::new(1, "root.txt"),
            SyncedPath::new(1, "test/elephant.txt"),
            SyncedPath::new(1, "invalid.xml"),
            SyncedPath::new(0, "bar/ok.pdf"),
            SyncedPath::new(0, "dummy/please.jpg"),
            SyncedPath::new(0, "bar/baz/drat.pdf"),
            SyncedPath::new(0, "foo/ignore.txt"),
        ]
    }

    #[test]
    fn print_tree() {
        let files = files();
        let printer = SyncedPathPrinter::from_iter(&files);
        println!("{printer}");

        assert_eq!(
            printer.to_string(),
            "ROOT
├── 0
│   ├── asdf.xml
│   ├── bar
│   │   ├── baz
│   │   │   ├── drat.pdf
│   │   │   └── random.txt
│   │   └── ok.pdf
│   ├── dummy
│   │   ├── err.pdf
│   │   └── please.jpg
│   └── foo
│       └── ignore.txt
└── 1
    ├── invalid.xml
    ├── root.txt
    └── test
        ├── country
        │   └── word.txt
        ├── elephant.txt
        ├── hello
        │   ├── tree.txt
        │   └── world.txt
        ├── house.txt
        └── mouse.txt\n"
        );
    }

    #[test]
    fn print_tree_with_extras() {
        let files = files();
        let mut files: Vec<_> = files.into_iter().map(|x| (x, " -> data")).collect();
        files[2].1 = " -> other data";
        files[5].1 = "_hello_world";
        files[12].1 = " -> +tag1 -tag2";

        let printer: SyncedPathPrinter<&str> = files.iter().map(|(a, b)| (a, *b)).collect();

        println!("{printer}");

        assert_eq!(
            printer.to_string(),
            "ROOT
├── 0
│   ├── asdf.xml -> data
│   ├── bar
│   │   ├── baz
│   │   │   ├── drat.pdf -> data
│   │   │   └── random.txt -> data
│   │   └── ok.pdf -> data
│   ├── dummy
│   │   ├── err.pdf -> data
│   │   └── please.jpg -> +tag1 -tag2
│   └── foo
│       └── ignore.txt -> data
└── 1
    ├── invalid.xml -> data
    ├── root.txt -> data
    └── test
        ├── country
        │   └── word.txt -> data
        ├── elephant.txt -> data
        ├── hello
        │   ├── tree.txt_hello_world
        │   └── world.txt -> data
        ├── house.txt -> data
        └── mouse.txt -> other data\n"
        );
    }

    #[test]
    fn print_tree_with_empty_filename() {
        let files = [SyncedPath::new(0, ""), SyncedPath::new(1, "")];
        let printer = SyncedPathPrinter::from_iter(&files);
        println!("{printer}");
        assert_eq!(
            printer.to_string(),
            "ROOT
├── 0
│   └── 
└── 1
    └── 
"
        );
    }

    #[test]
    fn last_n_chars() {
        let world = take_last_n_chars("Hello World", 5);
        assert_eq!(world, "World");
    }

    #[test]
    fn last_n_chars_empty() {
        let empty = take_last_n_chars("", 5);
        assert_eq!(empty, "");
    }
}
