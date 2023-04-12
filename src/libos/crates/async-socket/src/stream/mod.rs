mod states;

use self::states::{ConnectedStream, ConnectingStream, InitStream, ListenerStream};
use crate::common::Common;
use crate::ioctl::*;
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::sockopt::*;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_io::socket::{
    timeout_to_timeval, GetRecvTimeoutCmd, GetSendTimeoutCmd, MsgFlags, SetRecvTimeoutCmd,
    SetSendTimeoutCmd,
};

lazy_static! {
    pub static ref SEND_BUF_SIZE: AtomicUsize = AtomicUsize::new(2565 * 1024); // Default Linux send buffer size is 2.5MB.
    pub static ref RECV_BUF_SIZE: AtomicUsize = AtomicUsize::new(128 * 1024);
}

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

    pub fn new_pair(nonblocking: bool) -> Result<(Self, Self)> {
        let (common1, common2) = Common::new_pair(Type::STREAM, nonblocking)?;
        let connected1 = ConnectedStream::new(Arc::new(common1));
        let connected2 = ConnectedStream::new(Arc::new(common2));
        let socket1 = Self::new_connected(connected1);
        let socket2 = Self::new_connected(connected2);
        Ok((socket1, socket2))
    }

    fn new_connected(connected_stream: Arc<ConnectedStream<A, R>>) -> Self {
        let state = RwLock::new(State::Connected(connected_stream));
        Self { state }
    }

    fn try_switch_to_connected_state(
        connecting_stream: &Arc<ConnectingStream<A, R>>,
    ) -> Option<Arc<ConnectedStream<A, R>>> {
        // Previously, I thought connecting state only exists for non-blocking socket. However, some applications can set non-blocking for
        // connect syscall and after the connect returns, set the socket to blocking socket. Thus, this function shouldn't assert the connecting
        // stream is non-blocking socket.
        if connecting_stream.check_connection() {
            let common = connecting_stream.common().clone();
            common.set_peer_addr(connecting_stream.peer_addr());
            Some(ConnectedStream::new(common))
        } else {
            None
        }
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
                State::Connect(connecting_stream) => {
                    if let Some(connected_stream) =
                        Self::try_switch_to_connected_state(connecting_stream)
                    {
                        *state = State::Connected(connected_stream);
                        return_errno!(EISCONN, "the socket is already connected");
                    } else {
                        // Not connected, keep the connecting state and try connect
                        let init_stream =
                            InitStream::new_with_common(connecting_stream.common().clone())?;
                        (init_stream, connecting_stream.clone())
                    }
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

        // If success, then the state is switched to connected; otherwise, for blocking socket
        // the state is restored to the init state, and for non-blocking socket, the state
        // keeps in connecting state.
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
            Err(_) => {
                if !connecting_stream.common().nonblocking() {
                    let mut state = self.state.write().unwrap();
                    *state = State::Init(init_stream);
                }
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

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let ret = self.recvmsg(bufs, RecvFlags::empty()).await?;
        Ok(ret.0)
    }

    /// Receive messages from connected socket
    ///
    /// Linux behavior:
    /// Unlike datagram socket, `recvfrom` / `recvmsg` of stream socket will
    /// ignore the address even if user specified it.
    pub async fn recvmsg(
        &self,
        buf: &mut [&mut [u8]],
        flags: RecvFlags,
    ) -> Result<(usize, Option<A>, MsgFlags)> {
        let connected_stream = {
            let mut state = self.state.write().unwrap();
            match &*state {
                State::Connected(connected_stream) => connected_stream.clone(),
                State::Connect(connecting_stream) => {
                    if let Some(connected_stream) =
                        Self::try_switch_to_connected_state(connecting_stream)
                    {
                        *state = State::Connected(connected_stream.clone());
                        connected_stream
                    } else {
                        return_errno!(ENOTCONN, "the socket is not connected");
                    }
                }
                _ => {
                    return_errno!(ENOTCONN, "the socket is not connected");
                }
            }
        };

        let recv_len = connected_stream.recvmsg(buf, flags).await?;
        Ok((recv_len, None, MsgFlags::empty()))
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        self.writev(&[buf]).await
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.sendmsg(bufs, SendFlags::empty()).await
    }

    pub async fn sendmsg(&self, bufs: &[&[u8]], flags: SendFlags) -> Result<usize> {
        let connected_stream = {
            let mut state = self.state.write().unwrap();
            match &*state {
                State::Connected(connected_stream) => connected_stream.clone(),
                State::Connect(connecting_stream) => {
                    if let Some(connected_stream) =
                        Self::try_switch_to_connected_state(connecting_stream)
                    {
                        *state = State::Connected(connected_stream.clone());
                        connected_stream
                    } else {
                        return_errno!(ENOTCONN, "the socket is not connected");
                    }
                }
                _ => {
                    return_errno!(EPIPE, "the socket is not connected");
                }
            }
        };

        connected_stream.sendmsg(bufs, flags).await
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
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

        // Always get addr from host.
        // Because for IP socket, users can specify "0" as port and the kernel should select a usable port for him.
        // Thus, when calling getsockname, this should be updated.
        let addr = common.get_addr_from_host()?;
        common.set_addr(&addr);
        Ok(addr)
    }

    pub fn peer_addr(&self) -> Result<A> {
        let mut state = self.state.write().unwrap();
        match &*state {
            State::Connected(connected_stream) => {
                Ok(connected_stream.common().peer_addr().unwrap())
            }
            State::Connect(connecting_stream) => {
                if let Some(connected_stream) =
                    Self::try_switch_to_connected_state(connecting_stream)
                {
                    *state = State::Connected(connected_stream.clone());
                    Ok(connected_stream.common().peer_addr().unwrap())
                } else {
                    return_errno!(ENOTCONN, "the socket is not connected");
                }
            }
            _ => return_errno!(ENOTCONN, "the socket is not connected"),
        }
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        let mut state = self.state.write().unwrap();
        match &*state {
            State::Connect(connecting_stream) => {
                if let Some(connected_stream) =
                    Self::try_switch_to_connected_state(connecting_stream)
                {
                    *state = State::Connected(connected_stream.clone());
                }
            }
            _ => {}
        }
        drop(state);
        async_io::match_ioctl_cmd_mut!(&mut *cmd, {
            cmd: GetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetRecvTimeoutCmd => {
                self.set_recv_timeout(*cmd.timeout());
            },
            cmd: SetSendTimeoutCmd => {
                self.set_send_timeout(*cmd.timeout());
            },
            cmd: GetRecvTimeoutCmd => {
                let timeval = timeout_to_timeval(self.recv_timeout());
                cmd.set_output(timeval);
            },
            cmd: GetSendTimeoutCmd => {
                let timeval = timeout_to_timeval(self.send_timeout());
                cmd.set_output(timeval);
            },
            cmd: GetAcceptConnCmd => {
                let mut is_listen = false;
                let state = self.state.read().unwrap();
                if let State::Listen(_listener_stream) = &*state {
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
            cmd: SetNonBlocking => {
                let state = self.state.read().unwrap();
                state.common().set_nonblocking(*cmd.input() != 0);
            },
            cmd: GetReadBufLen => {
                let state = self.state.read().unwrap();
                if let State::Connected(connected_stream) = &*state {
                    let read_buf_len = connected_stream.bytes_to_consume();
                    cmd.set_output(read_buf_len as _);
                } else {
                    return_errno!(ENOTCONN, "unconnected socket");
                }
            },
            cmd: GetIfReqWithRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: GetIfConf => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetSndBufSizeCmd => {
                cmd.update_host(self.host_fd())?;
                let buf_size = cmd.buf_size();
                self.set_kernel_send_buf_size(buf_size);
            },
            cmd: SetRcvBufSizeCmd => {
                cmd.update_host(self.host_fd())?;
                let buf_size = cmd.buf_size();
                self.set_kernel_recv_buf_size(buf_size);
            },
            cmd: GetSndBufSizeCmd => {
                let buf_size = SEND_BUF_SIZE.load(Ordering::Relaxed);
                cmd.set_output(buf_size);
            },
            cmd: GetRcvBufSizeCmd => {
                let buf_size = RECV_BUF_SIZE.load(Ordering::Relaxed);
                cmd.set_output(buf_size);
            },
            _ => {
                return_errno!(EINVAL, "Not supported yet");
            }
        });
        Ok(())
    }

    pub async fn shutdown(&self, shutdown: Shutdown) -> Result<()> {
        let mut state = self.state.write().unwrap();
        match &*state {
            State::Listen(listener_stream) => {
                // listening socket can be shutdown and then re-use by calling listen again.
                listener_stream.shutdown(shutdown)?;
                if shutdown.should_shut_read() {
                    // Cancel pending accept requests. This is necessary because the socket is reusable.
                    listener_stream.cancel_accept_requests().await;
                    // Set init state
                    let init_stream =
                        InitStream::new_with_common(listener_stream.common().clone())?;
                    let init_state = State::Init(init_stream);
                    *state = init_state;
                    Ok(())
                } else {
                    // shutdown the writer of the listener expect to have no effect
                    Ok(())
                }
            }
            State::Connected(connected_stream) => connected_stream.shutdown(shutdown),
            State::Connect(connecting_stream) => {
                if let Some(connected_stream) =
                    Self::try_switch_to_connected_state(connecting_stream)
                {
                    connected_stream.shutdown(shutdown)?;
                    *state = State::Connected(connected_stream);
                    Ok(())
                } else {
                    return_errno!(ENOTCONN, "the socket is not connected");
                }
            }
            _ => {
                return_errno!(ENOTCONN, "the socket is not connected");
            }
        }
    }

    pub async fn close(&self) -> Result<()> {
        let state = self.state.read().unwrap();
        match &*state {
            State::Init(_) => {}
            State::Listen(listener_stream) => {
                listener_stream.common().set_closed();
                listener_stream.cancel_accept_requests().await;
            }
            State::Connect(connecting_stream) => {
                connecting_stream.common().set_closed();
                let need_wait = true;
                connecting_stream.cancel_connect_request(need_wait).await;
            }
            State::Connected(connected_stream) => {
                connected_stream.set_closed();
                connected_stream.cancel_recv_requests().await;
                connected_stream.try_empty_send_buf_when_close().await;
            }
        }
        Ok(())
    }

    fn send_timeout(&self) -> Option<Duration> {
        let state = self.state.read().unwrap();
        state.common().send_timeout()
    }

    fn recv_timeout(&self) -> Option<Duration> {
        let state = self.state.read().unwrap();
        state.common().recv_timeout()
    }

    fn set_send_timeout(&self, timeout: Duration) {
        let state = self.state.read().unwrap();
        state.common().set_send_timeout(timeout);
    }

    fn set_recv_timeout(&self, timeout: Duration) {
        let state = self.state.read().unwrap();
        state.common().set_recv_timeout(timeout);
    }

    fn set_kernel_send_buf_size(&self, buf_size: usize) {
        let state = self.state.read().unwrap();
        match &*state {
            State::Init(_) | State::Listen(_) | State::Connect(_) => {
                // The kernel buffer is only created when the socket is connected. Just update the static variable.
                SEND_BUF_SIZE.store(buf_size, Ordering::Relaxed);
            }
            State::Connected(connected_stream) => {
                connected_stream.try_update_send_buf_size(buf_size);
            }
        }
    }

    fn set_kernel_recv_buf_size(&self, buf_size: usize) {
        let state = self.state.read().unwrap();
        match &*state {
            State::Init(_) | State::Listen(_) | State::Connect(_) => {
                // The kernel buffer is only created when the socket is connected. Just update the static variable.
                RECV_BUF_SIZE.store(buf_size, Ordering::Relaxed);
            }
            State::Connected(connected_stream) => {
                connected_stream.try_update_recv_buf_size(buf_size);
            }
        }
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

impl<A: Addr + 'static, R: Runtime> Drop for StreamSocket<A, R> {
    fn drop(&mut self) {
        let state = self.state.read().unwrap();
        state.common().set_closed();
        drop(state);
    }
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
