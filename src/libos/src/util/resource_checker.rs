inventory::collect!(StaticResourceChecker);
pub struct StaticResourceChecker {
    is_leak: fn() -> bool,
}

impl StaticResourceChecker {
    pub fn new(is_leak: fn() -> bool) -> Self {
        Self { is_leak }
    }
    pub fn is_leak(&self) -> fn() -> bool {
        self.is_leak
    }
}
