//! Implement the NewType pattern with inherit_methods macro.

use inherit_methods_macro::inherit_methods;
use std::vec::Vec;

pub struct Stack<T>(Vec<T>);

// The following methods are inherited from Vec automatically
#[inherit_methods(from = "self.0")]
impl<T> Stack<T> {
    // Normal methods can be fixed with inherited methods in the same impl block.
    pub fn new() -> Self {
        Self(Vec::new())
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self(Vec::with_capacity(capacity))
    }

    // Inherited methods with all kinds of fucntion signature.
    fn push(&mut self, value: T);
    fn pop(&mut self) -> Option<T>;
    fn len(&self) -> usize;
    fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&T) -> bool;
}

#[test]
fn use_inherited_methods() {
    // As long as this code compiles, we are sure that the implementation of
    // inherited methods are generated. If the code runs successfully, we
    // can be sure that the generated implementation is correct.

    let mut stack = Stack::new();
    assert!(stack.len() == 0);
    stack.push(1);
    stack.push(2);
    stack.push(3);
    assert!(stack.len() == 3);
    assert!(stack.pop() == Some(3));
    assert!(stack.pop() == Some(2));
    assert!(stack.pop() == Some(1));
    assert!(stack.len() == 0);
}
