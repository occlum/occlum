use super::*;
use super::file::{File, FileRef};
use {std};

pub type FileDesc = u32;

#[derive(Debug, Default)]
#[repr(C)]
pub struct FileTable {
    table: Vec<Option<FileTableEntry>>,
    num_fds: usize,
}

#[derive(Debug, Clone)]
struct FileTableEntry {
    file: FileRef,
    close_on_spawn: bool,
}


impl FileTable {
    pub fn new() -> FileTable {
        FileTable {
            table: Vec::with_capacity(4),
            num_fds: 0,
        }
    }

    pub fn put(&mut self, file: FileRef, close_on_spawn: bool) -> FileDesc {
        let mut table = &mut self.table;

        let min_free_fd = if self.num_fds < table.len() {
            table.iter().enumerate().find(|&(idx, opt)| opt.is_none())
                .unwrap().0
        } else {
            table.push(None);
            table.len() - 1
        };

        table[min_free_fd as usize] = Some(FileTableEntry::new(file,
                                                               close_on_spawn));
        self.num_fds += 1;

        min_free_fd as FileDesc
    }

    pub fn put_at(&mut self, fd: FileDesc, file: FileRef, close_on_spawn: bool) {
        let mut table = &mut self.table;
        let mut table_entry = Some(FileTableEntry::new(file, close_on_spawn));
        if fd as usize >= table.len() {
            table.resize(fd as usize + 1, None);
        }
        std::mem::swap(&mut table_entry, &mut table[fd as usize]);
        if table_entry.is_none() {
            self.num_fds += 1;
        }
    }

    pub fn get(&self, fd: FileDesc) -> Result<FileRef, Error> {
        if fd as usize >= self.table.len() {
            return errno!(EBADF, "Invalid file descriptor");
        }

        let table = &self.table;
        match table[fd as usize].as_ref() {
            Some(table_entry) => Ok(table_entry.file.clone()),
            None => errno!(EBADF, "Invalid file descriptor"),
        }
    }

    pub fn del(&mut self, fd: FileDesc) -> Result<FileRef, Error> {
        if fd as usize >= self.table.len() {
            return errno!(EBADF, "Invalid file descriptor");
        }

        let mut del_table_entry = None;
        let table = &mut self.table;
        std::mem::swap(&mut del_table_entry, &mut table[fd as usize]);
        match del_table_entry {
            Some(del_table_entry) => {
                self.num_fds -= 1;
                Ok(del_table_entry.file)
            },
            None => errno!(EBADF, "Invalid file descriptor"),
        }
    }
}

impl Clone for FileTable {
    fn clone(&self) -> FileTable {
        // Only clone file descriptors that are not close-on-spawn
        let mut num_cloned_fds = 0;
        let cloned_table = self.table.iter().map(|entry| {
            match entry {
                Some(file_table_entry) => {
                    match file_table_entry.close_on_spawn {
                        false => {
                            num_cloned_fds += 1;
                            Some(file_table_entry.clone())
                        }
                        true => None
                    }
                },
                None => None
            }
        }).collect();

        FileTable {
            table: cloned_table,
            num_fds: num_cloned_fds,
        }
    }
}


impl FileTableEntry {
    fn new(file: FileRef, close_on_spawn: bool) -> FileTableEntry {
        FileTableEntry {
            file,
            close_on_spawn,
        }
    }
}
