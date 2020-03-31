use super::task::Task;
use super::*;
use fs::{File, FileRef, FileTable};
use vm::ProcessVM;

lazy_static! {
    // Dummy object to make all processes having a parent
    pub static ref IDLE_PROCESS: ProcessRef = {
        Arc::new(SgxMutex::new(Process {
            task: Default::default(),
            status: Default::default(),
            pid: 0,
            pgid: 1,
            tgid: 0,
            host_tid: 0,
            exit_status: 0,
            is_detached: false,
            cwd: "/".to_owned(),
            elf_path: "/".to_owned(),
            clear_child_tid: None,
            parent: None,
            children: Vec::new(),
            waiting_children: Default::default(),
            vm: Default::default(),
            file_table: Default::default(),
            rlimits: Default::default(),
        }))
    };
}

impl Process {
    // TODO: this constructor has become complicated enough to justify using builders
    pub fn new(
        cwd: &str,
        elf_path: &str,
        task: Task,
        vm_ref: ProcessVMRef,
        file_table_ref: FileTableRef,
        rlimits_ref: ResourceLimitsRef,
        is_detached: bool,
    ) -> Result<(pid_t, ProcessRef)> {
        let new_pid = process_table::alloc_pid();
        let new_process_ref = Arc::new(SgxMutex::new(Process {
            task: task,
            status: Default::default(),
            pid: new_pid,
            pgid: 1, // TODO: implement pgid
            tgid: new_pid,
            host_tid: 0,
            cwd: cwd.to_owned(),
            elf_path: elf_path.to_owned(),
            clear_child_tid: None,
            exit_status: 0,
            is_detached: is_detached,
            parent: None,
            children: Vec::new(),
            waiting_children: None,
            vm: vm_ref,
            file_table: file_table_ref,
            rlimits: rlimits_ref,
        }));
        Ok((new_pid, new_process_ref))
    }

    pub fn get_task(&self) -> &Task {
        &self.task
    }
    pub fn get_task_mut(&mut self) -> &mut Task {
        &mut self.task
    }
    /// pid as seen by the user is actually the thread group ID
    pub fn get_pid(&self) -> pid_t {
        self.tgid
    }
    /// tid as seen by the user is actually the process ID
    pub fn get_tid(&self) -> pid_t {
        self.pid
    }
    pub fn get_pgid(&self) -> pid_t {
        self.pgid
    }
    pub fn get_host_tid(&self) -> pid_t {
        self.host_tid
    }
    pub fn set_host_tid(&mut self, host_tid: pid_t) {
        self.host_tid = host_tid;
    }
    pub fn get_status(&self) -> Status {
        self.status
    }
    pub fn get_exit_status(&self) -> i32 {
        self.exit_status
    }
    pub fn get_cwd(&self) -> &str {
        &self.cwd
    }
    pub fn get_elf_path(&self) -> &str {
        &self.elf_path
    }
    pub fn get_vm(&self) -> &ProcessVMRef {
        &self.vm
    }
    pub fn get_files(&self) -> &FileTableRef {
        &self.file_table
    }
    pub fn get_parent(&self) -> &ProcessRef {
        self.parent.as_ref().unwrap()
    }
    pub fn get_children_iter(&self) -> impl Iterator<Item = ProcessRef> + '_ {
        self.children
            .iter()
            .filter_map(|child_weak| child_weak.upgrade())
    }
    pub fn change_cwd(&mut self, path: &str) {
        if path.len() > 0 && path.as_bytes()[0] == b'/' {
            // absolute
            self.cwd = path.to_owned();
        } else {
            // relative
            if !self.cwd.ends_with("/") {
                self.cwd += "/";
            }
            self.cwd += path;
        }
    }
    pub fn get_rlimits(&self) -> &ResourceLimitsRef {
        &self.rlimits
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        process_table::free_pid(self.pid);
    }
}

unsafe impl Send for Process {}
unsafe impl Sync for Process {}

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
