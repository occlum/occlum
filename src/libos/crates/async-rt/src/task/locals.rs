// This file is adapted from async_std

use core::cell::UnsafeCell;

use crate::prelude::*;

#[derive(Debug)]
pub struct LocalKey<T: Send + 'static> {
    init: fn() -> T,
    key: AtomicU32,
}

impl<T: Send + 'static> LocalKey<T> {
    pub const fn new(init: fn() -> T) -> Self {
        let key = AtomicU32::new(0);
        Self { init, key }
    }
}

impl<T: Send + 'static> LocalKey<T> {
    /// Attempts to get a reference to the task-local value with this key.
    ///
    /// This method will panic if not called within the context of a task.
    pub fn with<'a, F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&'a T) -> R,
    {
        self.try_with(f).unwrap()
    }

    /// Attempts to get a reference to the task-local value with this key.
    ///
    /// This method will not invoke the closure and return an `None` if not
    /// called within the context of a task.
    pub fn try_with<'a, F, R>(&'static self, f: F) -> Option<R>
    where
        F: FnOnce(&'a T) -> R,
    {
        let current = match crate::task::current::try_get() {
            Some(current) => current,
            None => return None,
        };

        // Prepare the numeric key, initialization function, and the map of task-locals.
        let key = self.key();
        let init = || Box::new((self.init)()) as Box<dyn Send>;

        // Get the value in the map of task-locals, or initialize and insert one.
        let value: *const dyn Send = current.locals().get_or_insert(key, init);

        // Call the closure with the value passed as an argument.
        let retval = unsafe { f(&*(value as *const T)) };
        Some(retval)
    }

    /// Returns the numeric key associated with this task-local.
    #[inline]
    pub fn key(&self) -> u32 {
        #[cold]
        fn init(key: &AtomicU32) -> u32 {
            static COUNTER: AtomicU32 = AtomicU32::new(1);

            let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
            if counter > u32::max_value() / 2 {
                panic!("counter overflow");
            }

            match key.compare_exchange(0, counter, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => counter,
                Err(x) => x,
            }
        }

        match self.key.load(Ordering::Acquire) {
            0 => init(&self.key),
            k => k,
        }
    }
}

// A map that holds task-locals.
pub(crate) struct LocalsMap {
    /// A list of key-value entries sorted by the key.
    entries: UnsafeCell<Option<Vec<Entry>>>,
}

unsafe impl Send for LocalsMap {}

impl LocalsMap {
    /// Creates an empty map of task-locals.
    pub fn new() -> LocalsMap {
        LocalsMap {
            entries: UnsafeCell::new(Some(Vec::new())),
        }
    }

    /// Returns a task-local value associated with `key` or inserts one constructed by `init`.
    #[inline]
    pub fn get_or_insert(&self, key: u32, init: impl FnOnce() -> Box<dyn Send>) -> &dyn Send {
        match unsafe { (*self.entries.get()).as_mut() } {
            None => panic!("can't access task-locals while the task is being dropped"),
            Some(entries) => {
                let index = match entries.binary_search_by_key(&key, |e| e.key) {
                    Ok(i) => i,
                    Err(i) => {
                        let value = init();
                        entries.insert(i, Entry { key, value });
                        i
                    }
                };
                &*entries[index].value
            }
        }
    }

    /// Clears the map and drops all task-locals.
    ///
    /// This method is only safe to call at the end of the task.
    pub unsafe fn clear(&self) {
        // Since destructors may attempt to access task-locals, we musnt't hold a mutable reference
        // to the `Vec` while dropping them. Instead, we first take the `Vec` out and then drop it.
        let entries = (*self.entries.get()).take();
        drop(entries);
    }
}

/// A key-value entry in a map of task-locals.
struct Entry {
    /// Key identifying the task-local variable.
    key: u32,

    /// Value stored in this entry.
    value: Box<dyn Send>,
}
