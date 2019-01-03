use super::*;
use super::file::{File, FileRef};
use {std};

pub type FileDesc = u32;

// Invariant 1: fd < max_fd, where fd is any fd in the table
// Invariant 2: max_fd = table.size()
// Invariant 3: num_fds <= table.size()
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct FileTable {
    table: Vec<Option<FileRef>>,
    max_fd: FileDesc,
    num_fds: u32,
}

impl FileTable {
    pub fn new() -> FileTable {
        FileTable {
            table: Vec::with_capacity(0),
            max_fd: 0,
            num_fds: 0,
        }
    }

    pub fn put(&mut self, file: FileRef) -> FileDesc {
        let mut table = &mut self.table;

        let free_fd = if self.num_fds < self.max_fd {
            table.iter().enumerate()
                .find(|&(idx, opt)| opt.is_none()).unwrap().0 as FileDesc
        } else {
            table.push(None);
            self.max_fd += 1;
            self.num_fds
        };

        table[free_fd as usize] = Some(file);
        self.num_fds += 1;

        free_fd
    }

    pub fn get(&self, fd: FileDesc) -> Option<FileRef> {
        if fd >= self.max_fd {
            return None;
        }

        let table = &self.table;
        table[fd as usize].as_ref().map(|file_ref| file_ref.clone())
    }

    pub fn del(&mut self, fd: FileDesc) -> Option<FileRef> {
        if fd >= self.max_fd {
            return None;
        }

        let mut del_file = None;
        let table = &mut self.table;
        std::mem::swap(&mut del_file, &mut table[fd as usize]);
        if del_file.is_none() {
            return None;
        }

        self.num_fds -= 1;
        if fd + 1 == self.max_fd {
            self.max_fd = table.iter().enumerate().rev()
                .find(|&(idx, opt)| opt.is_some())
                .map_or(0, |(max_used_fd,opt)| max_used_fd + 1) as FileDesc;
        }
        del_file
    }
}
