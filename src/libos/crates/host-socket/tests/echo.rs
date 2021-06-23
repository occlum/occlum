use async_io::socket::{Addr, Ipv4Addr, Ipv4SocketAddr, UnixAddr};

mod runtime {
    use std::sync::Once;

    use host_socket::Runtime;
    use io_uring_callback::{Builder as IoUringBuilder, IoUring};

    pub struct SocketRuntime;

    impl SocketRuntime {
        pub fn init(parallelism: u32) {
            static INIT: Once = Once::new();
            INIT.call_once(|| {
                async_rt::config::set_parallelism(parallelism);

                let ring = Self::io_uring();
                unsafe {
                    ring.start_enter_syscall_thread();
                }
                let callback = move || {
                    ring.poll_completions();
                };
                async_rt::config::set_sched_callback(callback);
            });
        }
    }

    lazy_static::lazy_static! {
        static ref IO_URING: IoUring = IoUringBuilder::new().build(4096).unwrap();
    }

    impl Runtime for SocketRuntime {
        fn io_uring() -> &'static IoUring {
            &*IO_URING
        }
    }
}

mod server {
    use async_io::socket::Addr;
    use errno::prelude::*;
    use host_socket::StreamSocket;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::runtime::SocketRuntime;

    pub struct Builder<A: Addr + 'static> {
        addr: Option<A>,
        max_accept: Option<usize>,
    }

    impl<A: Addr + 'static> Builder<A> {
        pub fn new() -> Self {
            Self {
                addr: None,
                max_accept: None,
            }
        }

        /// The address that the server will be bound to.
        pub fn addr(mut self, addr: A) -> Self {
            self.addr = Some(addr);
            self
        }

        /// The max number of incoming sockets to accept.
        pub fn max_accept(mut self, max_accept: usize) -> Self {
            self.max_accept = Some(max_accept);
            self
        }

        pub fn build(self) -> Result<EchoServer<A>> {
            let remain_accept = {
                let max_accept = self.max_accept.unwrap_or(1);
                AtomicUsize::new(max_accept)
            };
            let socket = {
                let addr = self
                    .addr
                    .ok_or_else(|| errno!(EINVAL, "an address must be given"))?;
                let socket = StreamSocket::new()?;
                socket.bind(&addr)?;
                socket.listen(2)?;
                socket
            };
            let server = EchoServer {
                remain_accept,
                socket,
            };
            Ok(server)
        }
    }

    pub struct EchoServer<A: Addr + 'static> {
        remain_accept: AtomicUsize,
        socket: StreamSocket<A, SocketRuntime>,
    }

    impl<A: Addr + 'static> EchoServer<A> {
        pub async fn run(self) -> Result<()> {
            while self.remain_accept.load(Ordering::Relaxed) > 0 {
                let client_socket = self.socket.accept().await?;

                //async_rt::task::spawn(async move {
                let mut buf = vec![0u8; 4 * 1024];
                loop {
                    let read_buf = &mut buf[..];
                    let bytes_read = client_socket
                        .read(read_buf)
                        .await
                        .expect("client read failed");

                    if bytes_read == 0 {
                        //return;
                        break;
                    }

                    let mut write_buf = &read_buf[..bytes_read];
                    while write_buf.len() > 0 {
                        let bytes_write = client_socket
                            .write(write_buf)
                            .await
                            .expect("client write failed");
                        write_buf = &write_buf[bytes_write..];
                    }
                }
                //});

                self.remain_accept.fetch_sub(1, Ordering::Relaxed);
            }
            Ok(())
        }
    }
}

#[test]
fn ipv4() {
    runtime::SocketRuntime::init(1);

    let server_addr = {
        let ipv4_addr = Ipv4Addr::new(127, 0, 0, 1);
        let port = 9999;
        Ipv4SocketAddr::new(ipv4_addr, port)
    };

    // Spawn a task to run the server
    async_rt::task::block_on({
        let server_addr = server_addr.clone();
        async move {
            server::Builder::new()
                .addr(server_addr)
                .max_accept(1)
                .build()
                .expect("failed to init the server")
                .run()
                .await
                .expect("failed to run the server");
        }
    });
}

//#[test]
fn unix() {
    runtime::SocketRuntime::init(2);

    let path = "test.sock";
    std::fs::remove_file(&path).unwrap();
    let server_addr = UnixAddr::Pathname(path.to_string());

    // Spawn a task to run the server
    async_rt::task::block_on({
        let server_addr = server_addr.clone();
        async move {
            server::Builder::new()
                .addr(server_addr)
                .max_accept(1)
                .build()
                .expect("failed to init the server")
                .run()
                .await
                .expect("failed to run the server");
        }
    });
}
