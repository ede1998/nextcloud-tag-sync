#![no_main]

use std::{collections::HashMap, path::PathBuf};

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

use nextcloud_tag_sync::{
    Command, Modification, PrefixMapping, Repository, SyncedPath, SyncedPathPrinter, Tag,
    TagAction, Tags, in_memory_patch,
};

fuzz_target!(|data: ArbitraryFiles| {
    dbg!(&data);
    let mut cached = pretty_print(repo(&data, RepoKind::Cached));
    let local = pretty_print(repo(&data, RepoKind::Local));
    let remote = pretty_print(repo(&data, RepoKind::Remote));
    let expected = repo(&data, RepoKind::Outcome);

    let mut expected_local_commands = commands(&data, true);
    let mut expected_remote_commands = commands(&data, false);

    let (mut local_commands, mut remote_commands) = in_memory_patch(&mut cached, &local, &remote);

    sort(&mut local_commands);
    sort(&mut remote_commands);
    sort(&mut expected_local_commands);
    sort(&mut expected_remote_commands);

    assert_eq!(cached, expected, "Unexpected change in cached state.");
    assert_eq!(
        local_commands, expected_local_commands,
        "Unexpected local command."
    );
    assert_eq!(
        remote_commands, expected_remote_commands,
        "Unexpect remote command."
    );
});

fn pretty_print(repo: Repository) -> Repository {
    let x: SyncedPathPrinter<_> = repo
        .files()
        .iter()
        .map(|(path, tags)| (path, format!(": {tags}")))
        .collect();
    println!("{x}");
    repo
}

fn sort(commands: &mut [Command]) {
    for command in commands.iter_mut() {
        command.actions.sort_unstable();
    }
    commands.sort_unstable();
}

fn prefix_mapping() -> Vec<PrefixMapping> {
    vec![
        PrefixMapping::new(
            PathBuf::from("local-path"),
            PathBuf::from(format!("{}remote-path", PrefixMapping::EXPECTED_PREFIX)),
        )
        .expect("valid prefix mapping"),
    ]
}

fn repo(data: &ArbitraryFiles, kind: RepoKind) -> Repository {
    let mut repo = Repository::new(prefix_mapping());
    for file in &data.0 {
        let tags = merge_tags(&file.tags, kind);
        let path = SyncedPath::new(0, &file.path);
        repo.insert(path, tags);
    }
    repo
}

fn commands(data: &ArbitraryFiles, local: bool) -> Vec<Command> {
    use Modification::{Add, Remove};

    data.0
        .iter()
        .filter_map(|file| {
            let actions: Vec<_> = file
                .tags
                .iter()
                .filter_map(|(FuzzTag(tag), difference)| {
                    let make_action = |modification: Modification| {
                        Some(TagAction {
                            tag: tag.clone(),
                            modification,
                        })
                    };

                    match difference {
                        Difference::AddLocal if !local => make_action(Add),
                        Difference::RemoveLocal if !local => make_action(Remove),
                        Difference::AddRemote if local => make_action(Add),
                        Difference::RemoveRemote if local => make_action(Remove),
                        _ => None,
                    }
                })
                .collect();

            if actions.is_empty() {
                None
            } else {
                let path = SyncedPath::new(0, &file.path);
                Some(Command { path, actions })
            }
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
enum RepoKind {
    Cached = 0,
    Local = 1,
    Remote = 2,
    Outcome = 3,
}

fn merge_tags(tags: &HashMap<FuzzTag, Difference>, kind: RepoKind) -> Tags {
    let mut result = Tags::default();
    for (FuzzTag(tag), difference) in tags {
        #[rustfmt::skip]
        let exists_in = match difference {
            //                       Cached Local Remote Outcome
            Difference::None =>         [ true,  true,  true,  true],
            Difference::AddLocal =>     [false,  true, false,  true],
            Difference::RemoveLocal =>  [ true, false,  true, false],
            Difference::AddRemote =>    [false, false,  true,  true],
            Difference::RemoveRemote => [ true,  true, false, false],
            Difference::AddBoth =>      [false,  true,  true,  true],
            Difference::RemoveBoth =>   [ true, false, false, false],
        };
        if exists_in[kind as usize] {
            result.insert_one(tag.clone());
        }
    }
    result
}

/// Describes what happened since the last sync.
/// E.g. AddLocal means that the tag was added locally but not remotely.
#[derive(Debug, Arbitrary, Eq, PartialEq, Hash)]
enum Difference {
    None,
    AddLocal,
    RemoveLocal,
    AddRemote,
    RemoveRemote,
    AddBoth,
    RemoveBoth,
}

#[derive(Debug)]
struct ArbitraryFiles(Vec<ArbitraryFile>);

impl<'a> Arbitrary<'a> for ArbitraryFiles {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        let files: HashMap<_, _> = HashMap::arbitrary(u)?;
        Ok(Self(
            files
                .into_iter()
                .map(|(path, tags)| ArbitraryFile { path, tags })
                .collect(),
        ))
    }
}

#[derive(Debug, Arbitrary)]
struct ArbitraryFile {
    pub path: String,
    pub tags: HashMap<FuzzTag, Difference>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct FuzzTag(pub Tag);

impl<'a> Arbitrary<'a> for FuzzTag {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        static TAGS: std::sync::LazyLock<Vec<FuzzTag>> = std::sync::LazyLock::new(|| {
            [
                "amber",
                "ash",
                "asphalt",
                "auburn",
                "avocado",
                "aquamarine",
                "azure",
                "beige",
                "bisque",
                "black",
                "blue",
                "bone",
                "bordeaux",
                "brass",
                "bronze",
                "brown",
                "burgundy",
                "camel",
                "caramel",
                "canary",
                "celeste",
                "cerulean",
                "champagne",
                "charcoal",
                "chartreuse",
                "chestnut",
                "chocolate",
                "citron",
                "claret",
                "coal",
                "cobalt",
                "coffee",
                "coral",
                "corn",
                "cream",
                "crimson",
                "cyan",
                "denim",
                "desert",
                "ebony",
                "ecru",
                "emerald",
                "feldspar",
                "fuchsia",
                "gold",
                "gray",
                "green",
                "heather",
                "indigo",
                "ivory",
                "jet",
                "khaki",
                "lime",
                "magenta",
                "maroon",
                "mint",
                "navy",
                "olive",
                "orange",
                "pink",
                "plum",
                "purple",
                "red",
                "rust",
                "salmon",
                "sienna",
                "silver",
                "snow",
                "steel",
                "tan",
                "teal",
                "tomato",
                "violet",
                "white",
                "yellow",
            ]
            .into_iter()
            .map(|c| FuzzTag(c.parse().expect("valid tags")))
            .collect()
        });

        u.choose(&TAGS).cloned()
    }
}
