use std::fs::File;
use std::path::Path;

use crate::prelude::*;
use crate::HostDisk;

pub struct OpenOptions<D: HostDisk> {
    _dummy: core::marker::PhantomData<D>,
}

impl<D: HostDisk> OpenOptions<D> {
    /// Creates a blank new set of options ready for configuration.
    pub fn new() -> Self {
        todo!()
    }

    ///
    pub fn read(&mut self, read: bool) -> &mut Self {
        self
    }

    pub fn write(&mut self, write: bool) -> &mut Self {
        self
    }

    pub fn clear(&mut self, truncate: bool) -> &mut Self {
        self
    }

    pub fn create(&mut self, create: bool) -> &mut Self {
        self
    }

    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<D> {
        let file = todo!();
        D::new(self, file)
    }
}
