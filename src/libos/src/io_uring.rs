use io_uring_callback::{Builder, IoUring};

lazy_static::lazy_static! {
    pub static ref SINGLETON: IoUring = {
        let io_uring = Builder::new().build(256).unwrap();
        unsafe {
            io_uring.start_enter_syscall_thread();
        }
        io_uring
    };
}
