use io_uring_callback::{Builder, IoUring};

lazy_static::lazy_static! {
    pub static ref SINGLETON: IoUring = {
        let io_uring = Builder::new()
            .setup_sqpoll(Some(500/* ms */))
            .build(256)
            .unwrap();
        io_uring
    };
}
