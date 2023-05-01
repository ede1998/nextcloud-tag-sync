use std::hint::unreachable_unchecked;

pub trait IntoOk {
    fn into_ok(self) -> Self::T;
    type T;
}

impl<T> IntoOk for Result<T, std::convert::Infallible> {
    type T = T;
    fn into_ok(self) -> T {
        match self {
            Ok(o) => o,
            // safe because Infallible can never be instantiated
            Err(_) => unsafe { unreachable_unchecked() },
        }
    }
}

pub fn take_last_n_chars(string: &str, n: usize) -> &str {
    let len = string
        .char_indices()
        .rev()
        .nth(n - 1)
        .map_or(0, |(idx, _)| idx);
    // Safety: we just computed the index via `char_indices`.
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
            #[allow(dead_code)]
            pub fn into_inner(self) -> $type_name {
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
