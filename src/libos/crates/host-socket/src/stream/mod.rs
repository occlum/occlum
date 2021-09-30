mod states;

use self::states::{Common, ConnectedStream, ConnectingStream, InitStream, ListenerStream};
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::sockopt::*;

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
    pub fn new(nonblocking: bool) -> Result<Self> {
        let init_stream = InitStream::new(nonblocking)?;
        let init_state = State::Init(init_stream);
        Ok(Self {
            state: RwLock::new(init_state),
        })
    }

    fn new_connected(connected_stream: Arc<ConnectedStream<A, R>>) -> Self {
        let state = RwLock::new(State::Connected(connected_stream));
        Self { state }
    }

    pub fn domain(&self) -> Domain {
        A::domain()
    }

    pub fn host_fd(&self) -> HostFd {
        let state = self.state.read().unwrap();
        state.common().host_fd()
    }

    pub fn status_flags(&self) -> StatusFlags {
        // Only support O_NONBLOCK
        let state = self.state.read().unwrap();
        if state.common().nonblocking() {
            StatusFlags::O_NONBLOCK
        } else {
            StatusFlags::empty()
        }
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        // Only support O_NONBLOCK
        let state = self.state.read().unwrap();
        let nonblocking = new_flags.is_nonblocking();
        state.common().set_nonblocking(nonblocking);
        Ok(())
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

    pub async fn accept(&self, nonblocking: bool) -> Result<Self> {
        let listener_stream = {
            let state = self.state.read().unwrap();
            match &*state {
                State::Listen(listener_stream) => listener_stream.clone(),
                _ => {
                    return_errno!(EINVAL, "the socket is not listening");
                }
            }
        };

        let connected_stream = listener_stream.accept(nonblocking).await?;

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

    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        let state = self.state.read().unwrap();
        let pollee = state.common().pollee();
        pollee.poll(mask, poller)
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        let state = self.state.read().unwrap();
        let pollee = state.common().pollee();
        pollee.register_observer(observer, mask);
        Ok(())
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        let state = self.state.read().unwrap();
        let pollee = state.common().pollee();
        pollee
            .unregister_observer(observer)
            .ok_or_else(|| errno!(ENOENT, "the observer is not registered"))
    }

    pub fn addr(&self) -> Result<A> {
        let state = self.state.read().unwrap();
        let common = state.common();
        match &*state {
            State::Init(_) | State::Connect(_) => Ok(common.addr().unwrap_or_default()),
            State::Connected(_) | State::Listen(_) => common.addr().map_or_else(
                || {
                    let addr = common.get_addr_from_host()?;
                    common.set_addr(&addr);
                    Ok(addr)
                },
                |addr| Ok(addr),
            ),
        }
    }

    pub fn peer_addr(&self) -> Result<A> {
        let state = self.state.read().unwrap();
        match &*state {
            State::Connected(connected_stream) => {
                Ok(connected_stream.common().peer_addr().unwrap())
            }
            _ => return_errno!(ENOTCONN, "the socket is not connected"),
        }
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_mut!(&mut *cmd, {
            cmd: GetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: GetAcceptConnCmd => {
                let mut is_listen = false;
                let state = self.state.read().unwrap();
                if let State::Listen(listener_stream) = &*state {
                    is_listen = true;
                }
                cmd.set_output(is_listen as _);
            },
            cmd: GetDomainCmd => {
                cmd.set_output(self.domain() as _);
            },
            cmd: GetPeerNameCmd => {
                let peer = self.peer_addr()?;
                cmd.set_output(AddrStorage(peer.to_c_storage()));
            },
            cmd: GetTypeCmd => {
                let state = self.state.read().unwrap();
                cmd.set_output(state.common().type_() as _);
            },
            _ => {
                return_errno!(EINVAL, "Not supported yet");
            }
        });
        Ok(())
    }

    pub fn shutdown(&self, shutdown: Shutdown) -> Result<()> {
        let connected_stream = {
            let state = self.state.read().unwrap();
            match &*state {
                State::Connected(connected_stream) => connected_stream.clone(),
                _ => {
                    return_errno!(ENOTCONN, "the socket is not connected");
                }
            }
        };

        connected_stream.shutdown(shutdown)
    }

    /*
        pub fn poll_by(&self, mask: Events, mut poller: Option<&mut Poller>) -> Events {
            let state = self.state.read();
            match *state {
                Init(init_stream) => init_stream.poll_by(mask, poller),
                Connect(connect_stream) => connect_stream.poll_by(mask, poller),
                Connected(connected_stream) = connected_stream.poll_by(mask, poller),
                Listen(listener_stream) = listener_stream.poll_by(mask, poller),
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

impl<A: Addr + 'static, R: Runtime> State<A, R> {
    fn common(&self) -> &Common<A, R> {
        match self {
            Self::Init(stream) => stream.common(),
            Self::Connect(stream) => stream.common(),
            Self::Connected(stream) => stream.common(),
            Self::Listen(stream) => stream.common(),
        }
    }
}
