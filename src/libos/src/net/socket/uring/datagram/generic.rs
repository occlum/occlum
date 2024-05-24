use core::time::Duration;

use crate::{
    events::{Observer, Poller},
    fs::{IoNotifier, StatusFlags},
    match_ioctl_cmd_mut,
    net::socket::MsgFlags,
};

use super::*;
use crate::fs::IoEvents as Events;
use crate::fs::{GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, IoctlCmd};

pub struct DatagramSocket<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    state: RwLock<State>,
    sender: Arc<Sender<A, R>>,
    receiver: Arc<Receiver<A, R>>,
}

impl<A: Addr, R: Runtime> DatagramSocket<A, R> {
    pub fn new(nonblocking: bool) -> Result<Self> {
        let common = Arc::new(Common::new(SocketType::DGRAM, nonblocking, None)?);
        let state = RwLock::new(State::new());
        let sender = Sender::new(common.clone());
        let receiver = Receiver::new(common.clone());
        Ok(Self {
            common,
            state,
            sender,
            receiver,
        })
    }

    pub fn new_pair(nonblocking: bool) -> Result<(Self, Self)> {
        let (common1, common2) = Common::new_pair(SocketType::DGRAM, nonblocking)?;
        let socket1 = Self::new_connected(common1);
        let socket2 = Self::new_connected(common2);
        Ok((socket1, socket2))
    }

    fn new_connected(common: Common<A, R>) -> Self {
        let common = Arc::new(common);
        let state = RwLock::new(State::new_connected());
        let sender = Sender::new(common.clone());
        let receiver = Receiver::new(common.clone());
        receiver.initiate_async_recv();
        Self {
            common,
            state,
            sender,
            receiver,
        }
    }

    pub fn domain(&self) -> Domain {
        A::domain()
    }

    pub fn host_fd(&self) -> FileDesc {
        self.common.host_fd()
    }

    pub fn status_flags(&self) -> StatusFlags {
        // Only support O_NONBLOCK
        if self.common.nonblocking() {
            StatusFlags::O_NONBLOCK
        } else {
            StatusFlags::empty()
        }
    }

    pub fn set_status_flags(&self, new_flags: StatusFlags) -> Result<()> {
        // Only support O_NONBLOCK
        let nonblocking = new_flags.is_nonblocking();
        self.common.set_nonblocking(nonblocking);
        Ok(())
    }

    /// When creating a datagram socket, you can use `bind` to bind the socket
    /// to a address, hence another socket can send data to this address.
    ///
    /// Binding is divided into explicit and implicit. Invoking `bind` is
    /// explicit binding, while invoking `sendto` / `sendmsg` / `connect`
    /// will trigger implicit binding.
    ///
    /// Datagram sockets can only bind once. You should use explicit binding or
    /// just implicit binding. The explicit binding will failed if it happens after
    /// a implicit binding.
    pub fn bind(&self, addr: &A) -> Result<()> {
        let mut state = self.state.write().unwrap();
        if state.is_bound() {
            return_errno!(EINVAL, "The socket is already bound to an address");
        }

        do_bind(self.host_fd(), addr)?;

        self.common.set_addr(addr);
        state.mark_explicit_bind();
        // Start async recv after explicit binding or implicit binding
        self.receiver.initiate_async_recv();

        Ok(())
    }

    /// Datagram sockets provide only connectionless interactions, But datagram sockets
    /// can also use connect to associate a socket with a specific address.
    /// After connection, any data sent on the socket is automatically addressed to the
    /// connected peer, and only data received from that peer is delivered to the user.
    ///
    /// Unlike stream sockets, datagram sockets can connect multiple times. But the socket
    /// can only connect to one peer in the same time; a second connect will change the
    /// peer address, and a connect to a address with family AF_UNSPEC will dissolve the
    /// association ("disconnect" or "unconnect").
    ///
    /// Before connection you can only use `sendto` / `sendmsg` / `recvfrom` / `recvmsg`.
    /// Only after connection, you can use `read` / `recv` / `write` / `send`.
    /// And you can ignore the address in `sendto` / `sendmsg` if you just want to
    /// send data to the connected peer.
    ///
    /// Ref 1: http://osr507doc.xinuos.com/en/netguide/disockD.connecting_datagrams.html
    /// Ref 2: https://www.masterraghu.com/subjects/np/introduction/unix_network_programming_v1.3/ch08lev1sec11.html
    pub fn connect(&self, peer_addr: Option<&A>) -> Result<()> {
        let mut state = self.state.write().unwrap();

        // if previous peer.is_default() and peer_addr.is_none()
        // is unspec, so the situation exists that both
        // !state.is_connected() and peer_addr.is_none() are true.

        if let Some(peer) = peer_addr {
            do_connect(self.host_fd(), Some(peer))?;

            self.receiver.reset_shutdown();
            self.sender.reset_shutdown();
            self.common.set_peer_addr(peer);

            if peer.is_default() {
                state.mark_disconnected();
            } else {
                state.mark_connected();
            }
            if !state.is_bound() {
                state.mark_implicit_bind();
                // Start async recv after explicit binding or implicit binding
                self.receiver.initiate_async_recv();
            }

        // TODO: update binding address in some cases
        // For a ipv4 socket bound to 0.0.0.0 (INADDR_ANY), if you do connection
        // to 127.0.0.1 (Local IP address), the IP address of the socket will
        // change to 127.0.0.1 too. And if connect to non-local IP address, linux
        // will assign a address to the socket.
        // In both cases, we should update the binding address that we stored.
        } else {
            do_connect::<A>(self.host_fd(), None)?;

            self.common.reset_peer_addr();
            state.mark_disconnected();

            // TODO: clear binding in some cases.
            // Disconnect will effect the binding address. In Linux, for socket that
            // explicit bound to local IP address, disconnect will clear the binding address,
            // but leave the port intact. For socket with implicit bound, disconnect will
            // clear both the address and port.
        }
        Ok(())
    }

    // Close the datagram socket, cancel pending iouring requests
    pub fn close(&self) -> Result<()> {
        self.sender.shutdown();
        self.receiver.shutdown();
        self.common.set_closed();
        self.cancel_requests();
        Ok(())
    }

    /// Shutdown the udp socket. This syscall is very TCP-oriented, but it is also useful for udp socket.
    /// Not like tcp, shutdown does nothing on the wire, it only changes shutdown states.
    /// The shutdown states block the io-uring request of receiving or sending message.
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        let state = self.state.read().unwrap();
        if !state.is_connected() {
            return_errno!(ENOTCONN, "The udp socket is not connected");
        }
        drop(state);
        match how {
            Shutdown::Read => {
                self.common.host_shutdown(how)?;
                self.receiver.shutdown();
                self.common.pollee().add_events(Events::IN);
            }
            Shutdown::Write => {
                if self.sender.is_empty() {
                    self.common.host_shutdown(how)?;
                }
                self.sender.shutdown();
                self.common.pollee().add_events(Events::OUT);
            }
            Shutdown::Both => {
                self.common.host_shutdown(Shutdown::Read)?;
                if self.sender.is_empty() {
                    self.common.host_shutdown(Shutdown::Write)?;
                }
                self.receiver.shutdown();
                self.sender.shutdown();
                self.common
                    .pollee()
                    .add_events(Events::IN | Events::OUT | Events::HUP);
            }
        }
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.readv(&mut [buf])
    }

    pub fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let state = self.state.read().unwrap();
        drop(state);

        self.recvmsg(bufs, RecvFlags::empty(), None)
            .map(|(ret, ..)| ret)
    }

    /// You can not invoke `recvfrom` directly after creating a datagram socket.
    /// That is because `recvfrom` doesn't privide a implicit binding. If you
    /// don't do a explicit or implicit binding, the sender doesn't know where
    /// to send the data.
    pub fn recvmsg(
        &self,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
        control: Option<&mut [u8]>,
    ) -> Result<(usize, Option<A>, MsgFlags, usize)> {
        self.receiver.recvmsg(bufs, flags, control)
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize> {
        self.writev(&[buf])
    }

    pub fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.sendmsg(bufs, None, SendFlags::empty(), None)
    }

    pub fn sendmsg(
        &self,
        bufs: &[&[u8]],
        addr: Option<&A>,
        flags: SendFlags,
        control: Option<&[u8]>,
    ) -> Result<usize> {
        let state = self.state.read().unwrap();
        if addr.is_none() && !state.is_connected() {
            return_errno!(EDESTADDRREQ, "Destination address required");
        }

        drop(state);
        let res = if let Some(addr) = addr {
            self.sender.sendmsg(bufs, addr, flags, control)
        } else {
            let peer = self.common.peer_addr();
            if let Some(peer) = peer.as_ref() {
                self.sender.sendmsg(bufs, peer, flags, control)
            } else {
                return_errno!(EDESTADDRREQ, "Destination address required");
            }
        };

        let mut state = self.state.write().unwrap();
        if !state.is_bound() {
            state.mark_implicit_bind();
            // Start async recv after explicit binding or implicit binding
            self.receiver.initiate_async_recv();
        }

        res
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        let pollee = self.common.pollee();
        pollee.poll(mask, poller)
    }

    pub fn addr(&self) -> Result<A> {
        let common = &self.common;

        // Always get addr from host.
        // Because for IP socket, users can specify "0" as port and the kernel should select a usable port for him.
        // Thus, when calling getsockname, this should be updated.
        let addr = common.get_addr_from_host()?;
        common.set_addr(&addr);
        Ok(addr)
    }

    pub fn notifier(&self) -> &IoNotifier {
        let notifier = self.common.notifier();
        notifier
    }

    pub fn peer_addr(&self) -> Result<A> {
        let state = self.state.read().unwrap();
        if !state.is_connected() {
            return_errno!(ENOTCONN, "the socket is not connected");
        }
        Ok(self.common.peer_addr().unwrap())
    }

    pub fn errno(&self) -> Option<Errno> {
        self.common.errno()
    }

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        match_ioctl_cmd_mut!(&mut *cmd, {
            cmd: GetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetRecvTimeoutCmd => {
                self.set_recv_timeout(*cmd.input());
            },
            cmd: SetSendTimeoutCmd => {
                self.set_send_timeout(*cmd.input());
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
                // Datagram doesn't support listen
                cmd.set_output(0);
            },
            cmd: GetDomainCmd => {
                cmd.set_output(self.domain() as _);
            },
            cmd: GetErrorCmd => {
                let error: i32 = self.errno().map(|err| err as i32).unwrap_or(0);
                cmd.set_output(error);
            },
            cmd: GetPeerNameCmd => {
                let peer = self.peer_addr()?;
                cmd.set_output(AddrStorage(peer.to_c_storage()));
            },
            cmd: GetTypeCmd => {
                cmd.set_output(self.common.type_() as _);
            },
            cmd: GetIfReqWithRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: GetIfConf => {
                cmd.execute(self.host_fd())?;
            },
            cmd: GetReadBufLen => {
                let read_buf_len = self.receiver.ready_len();
                cmd.set_output(read_buf_len as _);
            },
            _ => {
                return_errno!(EINVAL, "Not supported yet");
            }
        });
        Ok(())
    }

    fn send_timeout(&self) -> Option<Duration> {
        self.common.send_timeout()
    }

    fn recv_timeout(&self) -> Option<Duration> {
        self.common.recv_timeout()
    }

    fn set_send_timeout(&self, timeout: Duration) {
        self.common.set_send_timeout(timeout);
    }

    fn set_recv_timeout(&self, timeout: Duration) {
        self.common.set_recv_timeout(timeout);
    }

    fn cancel_requests(&self) {
        self.receiver.cancel_recv_requests();
        self.sender.try_clear_msg_queue_when_close();
    }
}

impl<A: Addr + 'static, R: Runtime> Drop for DatagramSocket<A, R> {
    fn drop(&mut self) {
        self.common.set_closed();
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for DatagramSocket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatagramSocket")
            .field("common", &self.common)
            .finish()
    }
}

#[derive(Debug)]
struct State {
    bind_state: BindState,
    is_connected: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            bind_state: BindState::Unbound,
            is_connected: false,
        }
    }

    pub fn new_connected() -> Self {
        Self {
            bind_state: BindState::Unbound,
            is_connected: true,
        }
    }

    pub fn is_bound(&self) -> bool {
        self.bind_state.is_bound()
    }

    #[allow(dead_code)]
    pub fn is_explicit_bound(&self) -> bool {
        self.bind_state.is_explicit_bound()
    }

    #[allow(dead_code)]
    pub fn is_implicit_bound(&self) -> bool {
        self.bind_state.is_implicit_bound()
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected
    }

    pub fn mark_explicit_bind(&mut self) {
        self.bind_state = BindState::ExplicitBound;
    }

    pub fn mark_implicit_bind(&mut self) {
        self.bind_state = BindState::ImplicitBound;
    }

    pub fn mark_connected(&mut self) {
        self.is_connected = true;
    }

    pub fn mark_disconnected(&mut self) {
        self.is_connected = false;
    }
}

#[derive(Debug)]
enum BindState {
    Unbound,
    ExplicitBound,
    ImplicitBound,
}

impl BindState {
    pub fn is_bound(&self) -> bool {
        match self {
            Self::Unbound => false,
            _ => true,
        }
    }

    #[allow(dead_code)]
    pub fn is_explicit_bound(&self) -> bool {
        match self {
            Self::ExplicitBound => true,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_implicit_bound(&self) -> bool {
        match self {
            Self::ImplicitBound => true,
            _ => false,
        }
    }
}
