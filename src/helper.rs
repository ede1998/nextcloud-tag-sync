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
