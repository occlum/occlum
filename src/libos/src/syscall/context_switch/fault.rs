use super::Exception;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum Fault {
    Syscall,
    Interrupt,
    Exception(Exception),
}

impl Default for Fault {
    fn default() -> Self {
        Fault::Syscall
    }
}
