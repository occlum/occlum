use super::*;
use super::task::{Task};
use vm::{ProcessVM, VMRangeTrait};
use fs::{FileTable, File, FileRef};

#[allow(non_camel_case_types)]
pub type pid_t = u32;

#[derive(Debug)]
pub struct Process {
    task: Task,
    status: Status,
    pid: pid_t,
    tgid: pid_t,
    exit_status: i32,
    exec_path: String,
    vm: ProcessVM,
    file_table: FileTable,
}

pub type ProcessRef = Arc<SgxMutex<Process>>;

impl Process {
    pub fn new(exec_path: &str, task: Task, vm: ProcessVM, file_table: FileTable)
        -> Result<(pid_t, ProcessRef), Error>
    {
        let new_pid = process_table::alloc_pid();
        let new_process_ref = Arc::new(SgxMutex::new(Process {
            task: task,
            status: Default::default(),
            pid: new_pid,
            tgid: new_pid,
            exec_path: exec_path.to_owned(),
            exit_status: 0,
            vm: vm,
            file_table: file_table,
        }));
        Ok((new_pid, new_process_ref))
    }

    pub fn get_task(&self) -> &Task { &self.task }
    pub fn get_task_mut(&mut self) -> &mut Task { &mut self.task }
    pub fn get_pid(&self) -> pid_t { self.pid }
    pub fn get_tgid(&self) -> pid_t { self.tgid }
    pub fn get_status(&self) -> Status { self.status }
    pub fn get_exit_status(&self) -> i32 { self.exit_status }
    pub fn get_exec_path(&self) -> &str { &self.exec_path }
    pub fn get_vm(&self) -> &ProcessVM { &self.vm }
    pub fn get_vm_mut(&mut self) -> &mut ProcessVM { &mut self.vm }
    pub fn get_files(&self) -> &FileTable { &self.file_table }
    pub fn get_files_mut(&mut self) -> &mut FileTable { &mut self.file_table }

    pub fn exit(&mut self, exit_status: i32) {
        self.exit_status = exit_status;
        self.status = Status::ZOMBIE;
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        process_table::free_pid(self.pid);
    }
}


#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    RUNNING,
    INTERRUPTIBLE,
    ZOMBIE,
    STOPPED,
}

impl Default for Status {
    fn default() -> Status {
        Status::RUNNING
    }
}
