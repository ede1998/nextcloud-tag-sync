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

#[derive(Debug)]
pub struct ErrorCollection {
    sources: Vec<Box<dyn std::error::Error>>,
}

impl ErrorCollection {
    pub fn new<E>(err: E) -> Self
    where
        E: std::error::Error + 'static,
    {
        Self {
            sources: vec![err.into()],
        }
    }
}

impl std::fmt::Display for ErrorCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.sources.len() > 1 {
            writeln!(f, "Multiple errors occurred:")?;
        }
        for source in &self.sources {
            writeln!(f, "{source}")?;
        }
        Ok(())
    }
}

impl<const M: usize, E: std::error::Error + 'static> From<[E; M]> for ErrorCollection {
    fn from(value: [E; M]) -> Self {
        let sources = value.into_iter().map(Into::into).collect();
        Self { sources }
    }
}

impl<E1, E2> From<(E1, E2)> for ErrorCollection
where
    E1: std::error::Error + 'static,
    E2: std::error::Error + 'static,
{
    fn from(value: (E1, E2)) -> Self {
        Self {
            sources: vec![value.0.into(), value.1.into()],
        }
    }
}

impl snafu::Error for ErrorCollection {}

pub fn take_last_n_chars(string: &str, n: usize) -> &str {
    let len = string
        .char_indices()
        .rev()
        .nth(n - 1)
        .map_or(0, |(idx, _)| idx);
    // Safety: we just computed the index via `char_indices`.
    unsafe { string.get_unchecked(len..) }
}