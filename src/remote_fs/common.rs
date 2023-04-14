
macro_rules! newtype {
    ($name:ident) => {
        #[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
        pub struct $name(u64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }

        impl $name {
            pub fn into_inner(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self(value)
            }
        }

        impl std::str::FromStr for $name {
            type Err = <u64 as std::str::FromStr>::Err;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse().map(Self)
            }

        }
    };
}

newtype!(TagId);
newtype!(FileId);