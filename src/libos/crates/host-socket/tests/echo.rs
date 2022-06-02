//! Test sockets with an echo server.

use async_io::socket::{Addr, Ipv4Addr, Ipv4SocketAddr, UnixAddr};

#[test]
fn ipv4() {
    let server_addr = {
        let ipv4_addr = Ipv4Addr::new(127, 0, 0, 1);
        let port = 9999;
        Ipv4SocketAddr::new(ipv4_addr, port)
    };
    let num_clients = 8;
    let total_data = 8 * 1024 * 1024;
    let buf_size = 4 * 1024;
    run_echo_server_and_clients(server_addr, num_clients, total_data, buf_size);
}

#[test]
fn unix() {
    let server_addr = {
        let path = "test.sock";
        std::fs::remove_file(&path);
        UnixAddr::Pathname(path.to_string())
    };
    let num_clients = 2;
    let total_data = 1 * 1024 * 1024;
    let buf_size = 123;
    run_echo_server_and_clients(server_addr, num_clients, total_data, buf_size);
}

fn run_echo_server_and_clients<A: Addr + 'static>(
    // The server address
    server_addr: A,
    // The number of clients
    num_clients: usize,
    // The number of bytes to be sent by each client
    total_data: usize,
    // The buffer size of each individual read / write
    buf_size: usize,
) {
    runtime::SocketRuntime::init(2);

    // Create the server and spawn a task to run it
    let server = server::Builder::new()
        .addr(server_addr.clone())
        .max_accept(num_clients)
        .build()
        .expect("failed to init the server");
    async_rt::task::spawn({
        async move {
            server.run().await.expect("failed to run the server");
        }
    });

    // Spawn many tasks to run the clients
    async_rt::task::block_on(async move {
        use async_rt::task::JoinHandle;

        let task_handles: Vec<JoinHandle<()>> = (0..num_clients)
            .into_iter()
            .map(|_| {
                async_rt::task::spawn({
                    let server_addr = server_addr.clone();
                    async move {
                        client::Builder::new()
                            .addr(server_addr)
                            .buf_size(buf_size)
                            .total_data(total_data)
                            .build()
                            .expect("failed to build a client")
                            .run()
                            .await
                            .expect("failed to run a client");
                    }
                })
            })
            .collect();

        for handle in task_handles {
            handle.await;
        }
    });
}

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
                std::thread::spawn(move || loop {
                    ring.poll_completions(1, 5000);
                });
            });
        }
    }

    lazy_static::lazy_static! {
        static ref IO_URING: IoUring = IoUringBuilder::new()
            .setup_sqpoll(Some(500/* ms */))
            .build(4096)
            .unwrap();
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
            let remain_accept = self.max_accept.unwrap_or(1);
            let socket = {
                let addr = self
                    .addr
                    .ok_or_else(|| errno!(EINVAL, "an address must be given"))?;
                let socket = StreamSocket::new(false)?;
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
        remain_accept: usize,
        socket: StreamSocket<A, SocketRuntime>,
    }

    impl<A: Addr + 'static> EchoServer<A> {
        pub async fn run(mut self) -> Result<()> {
            while self.remain_accept > 0 {
                let client_socket = self.socket.accept(false).await?;

                async_rt::task::spawn(async move {
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
                });

                self.remain_accept -= 1;
            }
            Ok(())
        }
    }
}

mod client {
    use async_io::socket::Addr;
    use errno::prelude::*;
    use host_socket::StreamSocket;

    use super::random_base64::RandomBase64;
    use super::runtime::SocketRuntime;
    use super::stream_socket_ext::StreamSocketExt;

    pub struct Builder<A: Addr + 'static> {
        addr: Option<A>,
        total_data: Option<usize>,
        buf_size: Option<usize>,
    }

    impl<A: Addr + 'static> Builder<A> {
        pub const DEFAULT_TOTAL_DATA: usize = 1024 * 1024; // 1MB
        pub const DEFAULT_BUF_SIZE: usize = 4096; // 4KB

        pub fn new() -> Self {
            Self {
                addr: None,
                total_data: None,
                buf_size: None,
            }
        }

        pub fn addr(mut self, addr: A) -> Self {
            self.addr = Some(addr);
            self
        }

        pub fn total_data(mut self, total_data: usize) -> Self {
            self.total_data = Some(total_data);
            self
        }

        pub fn buf_size(mut self, buf_size: usize) -> Self {
            self.buf_size = Some(buf_size);
            self
        }

        pub fn build(self) -> Result<Client<A>> {
            let addr = self
                .addr
                .ok_or_else(|| errno!(EINVAL, "an address must be given"))?;
            let remain_data = self.total_data.unwrap_or(Self::DEFAULT_TOTAL_DATA);
            let buf_size = self.buf_size.unwrap_or(Self::DEFAULT_BUF_SIZE);
            let socket = StreamSocket::new(false)?;
            let random_base64 = RandomBase64::new();
            let client = Client {
                addr,
                remain_data,
                buf_size,
                socket,
                random_base64,
            };
            Ok(client)
        }
    }

    pub struct Client<A: Addr + 'static> {
        addr: A,
        remain_data: usize,
        buf_size: usize,
        socket: StreamSocket<A, SocketRuntime>,
        random_base64: RandomBase64,
    }

    impl<A: Addr + 'static> Client<A> {
        pub async fn run(mut self) -> Result<()> {
            self.socket
                .connect(&self.addr)
                .await
                .expect("failed to connect");

            let mut write_buf = vec![0u8; self.buf_size];
            let mut read_buf = vec![0u8; self.buf_size];
            while self.remain_data > 0 {
                let msg_len = self.remain_data.min(self.buf_size);

                let write_msg = &mut write_buf[..msg_len];
                self.gen_random_msg(write_msg);

                self.socket.write_exact(write_msg).await;

                let read_msg = &mut read_buf[..msg_len];
                self.socket.read_exact(read_msg).await;

                assert!(write_msg == read_msg);

                self.remain_data -= msg_len;
            }
            Ok(())
        }

        fn gen_random_msg(&mut self, msg: &mut [u8]) {
            for byte in msg {
                *byte = self.random_base64.next();
            }
        }
    }
}

mod stream_socket_ext {
    use futures::future::BoxFuture;
    use futures::prelude::*;

    use async_io::socket::Addr;
    use host_socket::StreamSocket;

    use super::runtime::SocketRuntime;

    pub trait StreamSocketExt {
        fn write_exact<'a>(&'a self, buf: &'a [u8]) -> BoxFuture<'a, ()>;
        fn read_exact<'a>(&'a self, buf: &'a mut [u8]) -> BoxFuture<'a, ()>;
    }

    impl<A: Addr + 'static> StreamSocketExt for StreamSocket<A, SocketRuntime> {
        fn write_exact<'a>(&'a self, mut buf: &'a [u8]) -> BoxFuture<'a, ()> {
            (async move {
                while buf.len() > 0 {
                    let nbytes = self.write(buf).await.expect("failed to write");
                    buf = &buf[nbytes..];
                }
            })
            .boxed()
        }

        fn read_exact<'a>(&'a self, mut buf: &'a mut [u8]) -> BoxFuture<'a, ()> {
            (async move {
                while buf.len() > 0 {
                    let nbytes = self.read(buf).await.expect("failed to read");
                    buf = &mut buf[nbytes..];
                }
            })
            .boxed()
        }
    }
}

mod random_base64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;

    use lazy_static::lazy_static;

    /// A random generator for Base64 bytes.
    ///
    /// The Base 64 characters are `A`-`Z`, `a`-`z`, `0`-`9`, `+`, and `/`.
    pub struct RandomBase64 {
        hasher: DefaultHasher,
    }

    impl RandomBase64 {
        pub fn new() -> Self {
            let mut new_self = Self {
                hasher: DefaultHasher::new(),
            };
            // Give the hasher a random "seed"
            let seed = &new_self as *const _ as usize;
            new_self.hasher.write_usize(seed);
            new_self
        }

        pub fn next(&mut self) -> u8 {
            lazy_static! {
                static ref ALPHABET: Box<[u8]> = {
                    let mut alphabet = Vec::with_capacity(64);
                    (0..26).for_each(|i| alphabet.push(b'A' + i));
                    (0..26).for_each(|i| alphabet.push(b'a' + i));
                    (0..10).for_each(|i| alphabet.push(b'0' + i));
                    alphabet.push(b'+');
                    alphabet.push(b'/');
                    alphabet.into_boxed_slice()
                };
            }
            let new_hash = self.hasher.finish() as usize;
            self.hasher.write_usize(new_hash);
            let random_idx = new_hash % 64;
            ALPHABET[random_idx]
        }
    }
}
