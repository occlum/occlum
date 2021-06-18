use super::*;

use crate::events::{Event, Notifier};

pub type FileDesc = u32;

#[derive(Debug)]
#[repr(C)]
pub struct FileTable {
    table: Vec<Option<FileTableEntry>>,
    num_fds: usize,
    notifier: FileTableNotifier,
}

impl FileTable {
    pub fn new() -> FileTable {
        FileTable {
            table: Vec::with_capacity(4),
            num_fds: 0,
            notifier: FileTableNotifier::new(),
        }
    }

    pub fn dup(
        &mut self,
        fd: FileDesc,
        min_fd: FileDesc,
        close_on_spawn: bool,
    ) -> Result<FileDesc> {
        let file_ref = self.get(fd)?;

        let min_fd = min_fd as usize;
        let min_free_fd = {
            let mut table = &mut self.table;

            // Make sure that min_fd does not exceed the capacity of the table
            if min_fd >= table.len() {
                let expand_size = min_fd - table.len() + 1;
                for _ in 0..expand_size {
                    table.push(None);
                }
            }

            let free_fd = table
                .iter()
                .enumerate()
                .skip(min_fd as usize)
                .find(|&(idx, opt)| opt.is_none());

            if let Some((index, _)) = free_fd {
                index
            } else {
                // Span table when no free fd is found
                table.push(None);
                table.len() - 1
            }
        } as FileDesc;

        self.put_at(min_free_fd, file_ref, close_on_spawn);

        Ok(min_free_fd)
    }

    pub fn put(&mut self, file: FileRef, close_on_spawn: bool) -> FileDesc {
        let mut table = &mut self.table;

        let min_free_fd = if self.num_fds < table.len() {
            table
                .iter()
                .enumerate()
                .find(|&(idx, opt)| opt.is_none())
                .unwrap()
                .0
        } else {
            table.push(None);
            table.len() - 1
        };

        table[min_free_fd as usize] = Some(FileTableEntry::new(file, close_on_spawn));
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

    pub fn fds(&self) -> Vec<FileDesc> {
        let table = &self.table;
        table
            .iter()
            .enumerate()
            .filter(|(_, opt)| opt.is_some())
            .map(|(idx, _)| idx as FileDesc)
            .collect()
    }

    pub fn get(&self, fd: FileDesc) -> Result<FileRef> {
        let entry = self.get_entry(fd)?;
        Ok(entry.file.clone())
    }

    pub fn get_entry(&self, fd: FileDesc) -> Result<&FileTableEntry> {
        if fd as usize >= self.table.len() {
            return_errno!(EBADF, "Invalid file descriptor");
        }

        let table = &self.table;
        match table[fd as usize].as_ref() {
            Some(table_entry) => Ok(table_entry),
            None => return_errno!(EBADF, "Invalid file descriptor"),
        }
    }

    pub fn get_entry_mut(&mut self, fd: FileDesc) -> Result<&mut FileTableEntry> {
        if fd as usize >= self.table.len() {
            return_errno!(EBADF, "Invalid file descriptor");
        }

        let table = &mut self.table;
        match table[fd as usize].as_mut() {
            Some(table_entry) => Ok(table_entry),
            None => return_errno!(EBADF, "Invalid file descriptor"),
        }
    }

    pub fn del(&mut self, fd: FileDesc) -> Result<FileRef> {
        if fd as usize >= self.table.len() {
            return_errno!(EBADF, "Invalid file descriptor");
        }

        let mut del_table_entry = None;
        let table = &mut self.table;
        std::mem::swap(&mut del_table_entry, &mut table[fd as usize]);
        match del_table_entry {
            Some(del_table_entry) => {
                self.num_fds -= 1;
                self.broadcast_del(fd);
                Ok(del_table_entry.file)
            }
            None => return_errno!(EBADF, "Invalid file descriptor"),
        }
    }

    /// Remove all the file descriptors
    pub fn del_all(&mut self) -> Vec<FileRef> {
        let mut deleted_fds = Vec::new();
        let mut deleted_files = Vec::new();
        for (fd, entry) in self
            .table
            .iter_mut()
            .filter(|entry| entry.is_some())
            .enumerate()
        {
            deleted_files.push(entry.as_ref().unwrap().file.clone());
            *entry = None;
            deleted_fds.push(fd as FileDesc);
        }
        self.num_fds = 0;
        for fd in deleted_fds {
            self.broadcast_del(fd);
        }
        deleted_files
    }

    /// Remove file descriptors that are close-on-spawn
    pub fn close_on_spawn(&mut self) -> Vec<FileRef> {
        let mut deleted_fds = Vec::new();
        let mut deleted_files = Vec::new();
        for (fd, entry) in self.table.iter_mut().enumerate() {
            let need_close = if let Some(entry) = entry {
                entry.close_on_spawn
            } else {
                false
            };
            if need_close {
                deleted_files.push(entry.as_ref().unwrap().file.clone());
                *entry = None;
                deleted_fds.push(fd as FileDesc);
                self.num_fds -= 1;
            }
        }

        for fd in deleted_fds {
            self.broadcast_del(fd);
        }
        deleted_files
    }

    pub fn notifier(&self) -> &FileTableNotifier {
        &self.notifier
    }

    fn broadcast_del(&self, fd: FileDesc) {
        let del_event = FileTableEvent::Del(fd);
        self.notifier.broadcast(&del_event);
    }
}

impl Clone for FileTable {
    fn clone(&self) -> Self {
        FileTable {
            table: self.table.clone(),
            num_fds: self.num_fds,
            notifier: FileTableNotifier::new(),
        }
    }
}

impl Default for FileTable {
    fn default() -> Self {
        FileTable::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FileTableEvent {
    Del(FileDesc),
}

impl Event for FileTableEvent {}

pub type FileTableNotifier = Notifier<FileTableEvent>;

#[derive(Debug, Clone)]
pub struct FileTableEntry {
    file: FileRef,
    close_on_spawn: bool,
}

impl FileTableEntry {
    pub fn new(file: FileRef, close_on_spawn: bool) -> FileTableEntry {
        FileTableEntry {
            file,
            close_on_spawn,
        }
    }

    pub fn get_file(&self) -> &FileRef {
        &self.file
    }

    pub fn is_close_on_spawn(&self) -> bool {
        self.close_on_spawn
    }

    pub fn get_file_mut(&mut self) -> &mut FileRef {
        &mut self.file
    }

    pub fn set_close_on_spawn(&mut self, close_on_spawn: bool) {
        self.close_on_spawn = close_on_spawn;
    }
}
