mod states;

use std::sync::RwLock;

use async_io::socket::Addr;

use self::states::{Common, ConnectedStream, ConnectingStream, InitStream, ListenerStream};
use crate::prelude::*;
use crate::runtime::Runtime;

pub struct StreamSocket<A: Addr + 'static, R: Runtime> {
    state: RwLock<State<A, R>>,
}

enum State<A: Addr + 'static, R: Runtime> {
    // Start state
    Init(Arc<InitStream<A, R>>),
    // Intermediate state
    Connect(Arc<ConnectingStream<A, R>>),
    // Final state 1
    Connected(Arc<ConnectedStream<A, R>>),
    // Final state 2
    Listen(Arc<ListenerStream<A, R>>),
}

impl<A: Addr, R: Runtime> StreamSocket<A, R> {
    pub fn new() -> Result<Self> {
        let init_stream = InitStream::new()?;
        let init_state = State::Init(init_stream);
        Ok(Self {
            state: RwLock::new(init_state),
        })
    }

    fn new_connected(connected_stream: Arc<ConnectedStream<A, R>>) -> Self {
        let state = RwLock::new(State::Connected(connected_stream));
        Self { state }
    }

    pub fn bind(&self, addr: &A) -> Result<()> {
        let state = self.state.read().unwrap();
        match &*state {
            State::Init(init_stream) => init_stream.bind(addr),
            _ => {
                return_errno!(EINVAL, "cannot bind");
            }
        }
    }

    pub fn listen(&self, backlog: u32) -> Result<()> {
        let mut state = self.state.write().unwrap();
        match &*state {
            State::Init(init_stream) => {
                let common = init_stream.common().clone();
                let listener = ListenerStream::new(backlog, common)?;
                *state = State::Listen(listener);
                Ok(())
            }
            _ => {
                return_errno!(EINVAL, "cannot listen");
            }
        }
    }

    pub async fn connect(&self, peer_addr: &A) -> Result<()> {
        // Create the new intermediate state of connecting and save the
        // old state of init in case of failure to connect.
        let (init_stream, connecting_stream) = {
            let mut state = self.state.write().unwrap();
            match &*state {
                State::Init(init_stream) => {
                    let connecting_stream = {
                        let common = init_stream.common().clone();
                        ConnectingStream::new(peer_addr, common)?
                    };
                    let init_stream = init_stream.clone();
                    *state = State::Connect(connecting_stream.clone());
                    (init_stream, connecting_stream)
                }
                State::Connect(_) => {
                    return_errno!(EALREADY, "the socket is already connecting");
                }
                State::Connected(_) => {
                    return_errno!(EISCONN, "the socket is already connected");
                }
                State::Listen(_) => {
                    return_errno!(EINVAL, "the socket is listening");
                }
            }
        };

        let res = connecting_stream.connect().await;

        // If success, then the state transits to connected; otherwise,
        // the state is restored to the init state.
        match &res {
            Ok(()) => {
                let connected_stream = {
                    let common = init_stream.common().clone();
                    common.set_peer_addr(peer_addr);
                    ConnectedStream::new(common)
                };

                let mut state = self.state.write().unwrap();
                *state = State::Connected(connected_stream);
            }
            Err(e) => {
                let mut state = self.state.write().unwrap();
                *state = State::Init(init_stream);
            }
        }
        res
    }

    pub async fn accept(&self) -> Result<Self> {
        let listener_stream = {
            let state = self.state.read().unwrap();
            match &*state {
                State::Listen(listener_stream) => listener_stream.clone(),
                _ => {
                    return_errno!(EINVAL, "the socket is not listening");
                }
            }
        };

        let connected_stream = listener_stream.accept().await?;

        let new_self = Self::new_connected(connected_stream);
        Ok(new_self)
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.readv(&mut [buf]).await
    }

    pub async fn readv(&self, buf: &mut [&mut [u8]]) -> Result<usize> {
        let connected_stream = {
            let state = self.state.read().unwrap();
            match &*state {
                State::Connected(connected_stream) => connected_stream.clone(),
                _ => {
                    return_errno!(EINVAL, "the socket is not connected");
                }
            }
        };

        connected_stream.readv(buf).await
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        self.writev(&[buf]).await
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        let connected_stream = {
            let state = self.state.read().unwrap();
            match &*state {
                State::Connected(connected_stream) => connected_stream.clone(),
                _ => {
                    return_errno!(ENOTCONN, "the socket is not connected");
                }
            }
        };

        connected_stream.writev(bufs).await
    }

    /*
        pub async fn shutdown(&self, shutdown: Shutdown) -> Result<()> {
            let connected_stream = {
                let state = self.state.read();
                match *state {
                    Connected(connected_stream) => connected_stream.clone(),
                    _ => {
                        return_errno!(ENOTCONN, "the socket is not connected");
                    }
                }
            };

            connected_stream.shutdown(shutdown)
        }

        pub fn ioctl(&self, ioctl: &mut dyn IoctlCmd) -> Result<()> {
            return_errno!(EINVAL, "")
        }

        pub fn poll_by(&self, mask: Events, mut poller: Option<&mut Poller>) -> Events {
            let state = self.state.read();
            match *state {
                Init(init_stream) => init_stream.poll_by(mask, poller),
                Connect(connect_stream) => connect_stream.poll_by(mask, poller),
                Connected(connected_stream) = connected_stream.poll_by(mask, poller),
                Listen(listener_stream) = listener_stream.poll_by(mask, poller),
            }
        }

        pub fn addr(&self) -> Option<A> {
            let state = self.state.read().unwrap();
            match &*state {
                Init(init_stream) => init_stream.addr(),
                Connect(connecting_stream) => connecting_stream.addr(),
                Connected(connected_stream) = connected_stream.addr(),
                Listen(listener_stream) = listener_stream.addr(),
            }
        }

        pub fn peer_addr(&self) -> Option<A> {
            let state = self.state.read();
            match *state {
                Connected(connected_stream) = connected_stream.peer_addr(),
                _ => {
                    return_errno!(EINVAL, "")
                }
            }
        }
    */
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for State<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let inner: &dyn std::fmt::Debug = match self {
            State::Init(inner) => inner as _,
            State::Connect(inner) => inner as _,
            State::Connected(inner) => inner as _,
            State::Listen(inner) => inner as _,
        };
        f.debug_tuple("State").field(inner).finish()
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for StreamSocket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StreamSocket")
            .field("state", &self.state.read().unwrap())
            .finish()
    }
}
