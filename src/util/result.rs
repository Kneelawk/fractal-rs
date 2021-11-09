/// Extension trait for `Result` containing useful utility methods.
pub trait ResultExt<T, E> {
    /// Call a `FnOnce` if this is an error and return `None`, otherwise return
    /// a `Some` of the result value.
    fn on_err(self, on_err: impl FnOnce(E)) -> Option<T>;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn on_err(self, on_err: impl FnOnce(E)) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                on_err(e);
                None
            },
        }
    }
}
