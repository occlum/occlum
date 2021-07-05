//! Emulate "inheritance" with the inherit_methods macro.

use inherit_methods_macro::inherit_methods;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub trait Object {
    fn type_name(&self) -> &'static str;
    fn object_id(&self) -> u64;
    fn name(&self) -> String;
    fn set_name(&self, new_name: String);
}

struct ObjectBase {
    object_id: u64,
    name: Mutex<String>,
}

impl ObjectBase {
    pub fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        Self {
            object_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            name: Mutex::new(String::new()),
        }
    }

    pub fn object_id(&self) -> u64 {
        self.object_id
    }

    pub fn name(&self) -> String {
        self.name.lock().unwrap().clone()
    }

    pub fn set_name(&self, new_name: String) {
        *self.name.lock().unwrap() = new_name;
    }
}

struct DummyObject {
    base: ObjectBase,
}

impl DummyObject {
    pub fn new() -> Self {
        Self {
            base: ObjectBase::new(),
        }
    }
}

#[inherit_methods(from_field = "self.base")]
impl Object for DummyObject {
    fn type_name(&self) -> &'static str {
        "DummyObject"
    }

    // Inherit methods from the base class
    fn object_id(&self) -> u64;
    fn name(&self) -> String;
    fn set_name(&self, new_name: String);
}

#[test]
fn use_inherited_methods() {
    let dummy = DummyObject::new();
    assert!(dummy.object_id() == 0);
    assert!(&dummy.name() == "");

    let new_name = "this is dummy";
    dummy.set_name(new_name.to_string());
    assert!(&dummy.name() == new_name);
}
