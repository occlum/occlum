use super::*;
use crate::poll::EpollFile;
use std::sync::Weak;

pub type FileDesc = u32;

#[derive(Debug)]
#[repr(C)]
pub struct FileTable {
    table: Vec<Option<FileTableEntry>>,
    num_fds: usize,
}

impl FileTable {
    pub fn new() -> FileTable {
        FileTable {
            table: Vec::with_capacity(4),
            num_fds: 0,
        }
    }

    pub fn table(&self) -> &Vec<Option<FileTableEntry>> {
        &self.table
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

    pub fn put_at(&mut self, fd: FileDesc, file: FileRef, close_on_spawn: bool) -> Option<FileRef> {
        let mut table = &mut self.table;
        let mut table_entry = Some(FileTableEntry::new(file, close_on_spawn));
        if fd as usize >= table.len() {
            table.resize(fd as usize + 1, None);
        }
        std::mem::swap(&mut table_entry, &mut table[fd as usize]);
        if table_entry.is_none() {
            self.num_fds += 1;
        }
        table_entry.map(|entry| entry.file.clone())
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
                del_table_entry.remove_from_epoll(fd);
                Ok(del_table_entry.file)
            }
            None => return_errno!(EBADF, "Invalid file descriptor"),
        }
    }

    /// Remove all the file descriptors
    pub fn del_all(&mut self) -> Vec<FileRef> {
        let mut deleted_files = Vec::new();
        for (fd, entry) in self
            .table
            .iter_mut()
            .filter(|entry| entry.is_some())
            .enumerate()
        {
            let deleted_entry = entry.as_ref().unwrap();
            deleted_files.push(deleted_entry.file.clone());
            deleted_entry.remove_from_epoll(fd as FileDesc);
            *entry = None;
        }
        self.num_fds = 0;
        deleted_files
    }

    /// Remove file descriptors that are close-on-spawn
    pub fn close_on_spawn(&mut self) -> Vec<FileRef> {
        let mut deleted_files = Vec::new();
        for (fd, entry) in self.table.iter_mut().enumerate() {
            let need_close = if let Some(entry) = entry {
                entry.close_on_spawn
            } else {
                false
            };
            if need_close {
                let deleted_entry = entry.as_ref().unwrap();
                deleted_files.push(deleted_entry.file.clone());
                deleted_entry.remove_from_epoll(fd as FileDesc);
                *entry = None;
                self.num_fds -= 1;
            }
        }
        deleted_files
    }
}

impl Clone for FileTable {
    fn clone(&self) -> Self {
        FileTable {
            table: self.table.clone(),
            num_fds: self.num_fds,
        }
    }
}

impl Default for FileTable {
    fn default() -> Self {
        FileTable::new()
    }
}

#[derive(Debug)]
pub struct FileTableEntry {
    file: FileRef,
    close_on_spawn: bool,
    registered_epolls: Vec<Weak<EpollFile>>,
}

impl Clone for FileTableEntry {
    fn clone(&self) -> Self {
        Self {
            file: self.file.clone(),
            close_on_spawn: self.close_on_spawn,
            registered_epolls: Vec::new(),
        }
    }
}

impl FileTableEntry {
    pub fn new(file: FileRef, close_on_spawn: bool) -> FileTableEntry {
        FileTableEntry {
            file,
            close_on_spawn,
            registered_epolls: Vec::new(),
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

    pub fn register_epoll(&mut self, epoll_file: &Weak<EpollFile>) {
        self.registered_epolls.push(epoll_file.clone());
    }

    pub fn unregister_epoll(&mut self, epoll_file: &Weak<EpollFile>) {
        self.registered_epolls
            .retain(|item| !Weak::ptr_eq(epoll_file, item) && item.upgrade().is_some());
    }

    pub fn remove_from_epoll(&self, fd: FileDesc) {
        for epoll in self.registered_epolls.iter() {
            if let Some(epoll_file) = epoll.upgrade() {
                epoll_file.on_file_closed(fd);
            }
        }
    }
}
