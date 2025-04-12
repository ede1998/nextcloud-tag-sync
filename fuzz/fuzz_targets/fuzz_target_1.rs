#![feature(iter_intersperse)]
#![no_main]

use std::{collections::HashMap, path::PathBuf};

use nextcloud_tag_sync::{
    Command, Modification, PrefixMapping, Repository, SyncedPath, SyncedPathPrinter, Tag,
    TagAction, Tags, in_memory_patch,
};
use tracing_subscriber::EnvFilter;

use input::ArbitraryFiles;

libfuzzer_sys::fuzz_target!(
    init: {
        tracing_subscriber::fmt()
          .with_ansi(atty::is(atty::Stream::Stdout))
          .with_env_filter(EnvFilter::from_default_env())
          .init();
    },
    |data: ArbitraryFiles| {
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
    tracing::debug!("{x}");
    repo
}

fn sort(commands: &mut [Command]) {
    for command in commands.iter_mut() {
        command.actions.sort_unstable();
    }
    commands.sort_unstable();
}

fn repo(data: &ArbitraryFiles, kind: RepoKind) -> Repository {
    let mapping = vec![
        PrefixMapping::new(
            PathBuf::from("local-path"),
            PathBuf::from(format!("{}remote-path", PrefixMapping::EXPECTED_PREFIX)),
        )
        .expect("valid prefix mapping"),
    ];

    let mut repo = Repository::new(mapping);
    for file in &data.0 {
        let tags = merge_tags(&file.tags, kind);
        if tags.is_empty() {
            continue;
        }
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
                .filter_map(|(tag, difference)| {
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

fn merge_tags(tags: &HashMap<Tag, Difference>, kind: RepoKind) -> Tags {
    let mut result = Tags::default();
    for (tag, difference) in tags {
        #[rustfmt::skip]
        let exists_in = match difference {
            //                           Cached Local Remote Outcome
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
#[derive(Debug, arbitrary::Arbitrary, Eq, PartialEq, Hash)]
enum Difference {
    None,
    AddLocal,
    RemoveLocal,
    AddRemote,
    RemoveRemote,
    AddBoth,
    RemoveBoth,
}

mod input {
    use std::collections::HashMap;

    use arbitrary::Arbitrary;
    use nextcloud_tag_sync::Tag;

    use super::Difference;

    #[derive(Debug)]
    pub struct ArbitraryFiles(pub Vec<ArbitraryFile>);

    impl<'a> Arbitrary<'a> for ArbitraryFiles {
        fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
            let files: HashMap<ArbitraryPath, HashMap<FuzzTag, _>> = HashMap::arbitrary(u)?;
            Ok(Self(
                files
                    .into_iter()
                    .map(|(ArbitraryPath(path), tags)| ArbitraryFile {
                        path,
                        tags: tags.into_iter().map(|(t, d)| (t.0, d)).collect(),
                    })
                    .collect(),
            ))
        }
    }

    #[derive(Debug)]
    pub struct ArbitraryFile {
        pub path: String,
        pub tags: HashMap<Tag, Difference>,
    }

    #[derive(Debug, Clone, Hash, PartialEq, Eq)]
    struct FuzzTag(Tag);

    impl<'a> Arbitrary<'a> for FuzzTag {
        fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
            static TAGS: std::sync::LazyLock<Vec<FuzzTag>> = std::sync::LazyLock::new(|| {
                COLORS
                    .iter()
                    .map(|c| FuzzTag(c.parse().expect("valid tags")))
                    .collect()
            });

            u.choose(&TAGS).cloned()
        }
    }

    #[derive(Debug, Clone, Hash, PartialEq, Eq)]
    struct ArbitraryPath(String);

    impl<'a> Arbitrary<'a> for ArbitraryPath {
        fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
            let len = u.int_in_range(1..=8)?;
            let path = (0..len)
                .map(|_| u.choose(ANIMALS).copied())
                .intersperse(Ok(std::path::MAIN_SEPARATOR_STR))
                .collect::<Result<_, _>>()?;

            Ok(Self(path))
        }
    }

    const ANIMALS: &[&str] = &[
        "Aardvark",
        "Albatross",
        "Alligator",
        "Alpaca",
        "Ant",
        "Anteater",
        "Antelope",
        "Ape",
        "Armadillo",
        "Donkey",
        "Baboon",
        "Badger",
        "Barracuda",
        "Bat",
        "Bear",
        "Beaver",
        "Bee",
        "Bison",
        "Boar",
        "Buffalo",
        "Butterfly",
        "Camel",
        "Capybara",
        "Caribou",
        "Cassowary",
        "Cat",
        "Caterpillar",
        "Cattle",
        "Chamois",
        "Cheetah",
        "Chicken",
        "Chimpanzee",
        "Chinchilla",
        "Chough",
        "Clam",
        "Cobra",
        "Cockroach",
        "Cod",
        "Cormorant",
        "Coyote",
        "Crab",
        "Crane",
        "Crocodile",
        "Crow",
        "Curlew",
        "Deer",
        "Dinosaur",
        "Dog",
        "Dogfish",
        "Dolphin",
        "Dotterel",
        "Dove",
        "Dragonfly",
        "Duck",
        "Dugong",
        "Dunlin",
        "Eagle",
        "Echidna",
        "Eel",
        "Eland",
        "Elephant",
        "Elk",
        "Emu",
        "Falcon",
        "Ferret",
        "Finch",
        "Fish",
        "Flamingo",
        "Fly",
        "Fox",
        "Frog",
        "Gaur",
        "Gazelle",
        "Gerbil",
        "Giraffe",
        "Gnat",
        "Gnu",
        "Goat",
        "Goldfinch",
        "Goldfish",
        "Goose",
        "Gorilla",
        "Goshawk",
        "Grasshopper",
        "Grouse",
        "Guanaco",
        "Gull",
        "Hamster",
        "Hare",
        "Hawk",
        "Hedgehog",
        "Heron",
        "Herring",
        "Hippopotamus",
        "Hornet",
        "Horse",
        "Human",
        "Hummingbird",
        "Hyena",
        "Ibex",
        "Ibis",
        "Jackal",
        "Jaguar",
        "Jay",
        "Jellyfish",
        "Kangaroo",
        "Kingfisher",
        "Koala",
        "Kookabura",
        "Kouprey",
        "Kudu",
        "Lapwing",
        "Lark",
        "Lemur",
        "Leopard",
        "Lion",
        "Llama",
        "Lobster",
        "Locust",
        "Loris",
        "Louse",
        "Lyrebird",
        "Magpie",
        "Mallard",
        "Manatee",
        "Mandrill",
        "Mantis",
        "Marten",
        "Meerkat",
        "Mink",
        "Mole",
        "Mongoose",
        "Monkey",
        "Moose",
        "Mosquito",
        "Mouse",
        "Mule",
        "Narwhal",
        "Newt",
        "Nightingale",
        "Octopus",
        "Okapi",
        "Opossum",
        "Oryx",
        "Ostrich",
        "Otter",
        "Owl",
        "Oyster",
        "Panther",
        "Parrot",
        "Partridge",
        "Peafowl",
        "Pelican",
        "Penguin",
        "Pheasant",
        "Pig",
        "Pigeon",
        "Pony",
        "Porcupine",
        "Porpoise",
        "Quail",
        "Quelea",
        "Quetzal",
        "Rabbit",
        "Raccoon",
        "Rail",
        "Ram",
        "Rat",
        "Raven",
        "Red deer",
        "Red panda",
        "Reindeer",
        "Rhinoceros",
        "Rook",
        "Salamander",
        "Salmon",
        "Sand Dollar",
        "Sandpiper",
        "Sardine",
        "Scorpion",
        "Seahorse",
        "Seal",
        "Shark",
        "Sheep",
        "Shrew",
        "Skunk",
        "Snail",
        "Snake",
        "Sparrow",
        "Spider",
        "Spoonbill",
        "Squid",
        "Squirrel",
        "Starling",
        "Stingray",
        "Stinkbug",
        "Stork",
        "Swallow",
        "Swan",
        "Tapir",
        "Tarsier",
        "Termite",
        "Tiger",
        "Toad",
        "Trout",
        "Turkey",
        "Turtle",
        "Viper",
        "Vulture",
        "Wallaby",
        "Walrus",
        "Wasp",
        "Weasel",
        "Whale",
        "Wildcat",
        "Wolf",
        "Wolverine",
        "Wombat",
        "Woodcock",
        "Woodpecker",
        "Worm",
        "Wren",
        "Yak",
        "Zebra",
    ];

    const COLORS: &[&str] = &[
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
    ];
}
