/// Delay the execution of a closure until this wrapper object goes out of
/// the scope.
///
/// Delay is mainly used to trigger some kinds of clean-up logic automatically.
pub struct Delay<F: Fn() -> ()> {
    f: F,
}

impl<F: Fn() -> ()> Delay<F> {
    pub fn new(f: F) -> Self {
        Self { f }
    }
}

impl<F: Fn() -> ()> Drop for Delay<F> {
    fn drop(&mut self) {
        (self.f)();
    }
}
