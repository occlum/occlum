//! Datagram sockets.
mod receiver;
mod sender;

use self::receiver::Receiver;
use self::sender::Sender;
use crate::common::{do_bind, Common};
use crate::prelude::*;
use crate::runtime::Runtime;
use crate::sockopt::*;

const MAX_BUF_SIZE: usize = 64 * 1024;

pub struct DatagramSocket<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    state: RwLock<State>,
    sender: Sender<A, R>,
    receiver: Arc<Receiver<A, R>>,
}

impl<A: Addr, R: Runtime> DatagramSocket<A, R> {
    pub fn new(nonblocking: bool) -> Result<Self> {
        let common = Arc::new(Common::new(Type::DGRAM, nonblocking)?);
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
        let (common1, common2) = Common::new_pair(Type::DGRAM, nonblocking)?;
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

    pub fn host_fd(&self) -> HostFd {
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
    pub async fn connect(&self, peer_addr: Option<&A>) -> Result<()> {
        let mut state = self.state.write().unwrap();
        if !state.is_connected() && peer_addr.is_none() {
            return Ok(());
        }

        if let Some(peer) = peer_addr {
            // We don't do connect syscall (or send connect io-uring requests) actually,
            // We emulate the connect behavior by recording the peer address and applying
            // the peer address during sendmsg and recvmsg.
            //
            // The advantage of emulation is avoiding to design a intermediate state (connecting)
            // and avoiding to deal with some complex case. e.g, If we do connect actually,
            // disconnect or connect to new address might affect the ongoing async recv request,
            // which might increase the design complexity.

            self.common.set_peer_addr(peer);
            state.mark_connected();
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

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.readv(&mut [buf]).await
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        let state = self.state.read().unwrap();
        if !state.is_connected() {
            return_errno!(ENOTCONN, "the socket is not connected");
        }
        drop(state);

        self.recvmsg(bufs, RecvFlags::empty())
            .await
            .map(|(ret, ..)| ret)
    }

    /// You can not invoke `recvfrom` directly after creating a datagram socket.
    /// That is because `recvfrom` doesn't privide a implicit binding. If you
    /// don't do a explicit or implicit binding, the sender doesn't know where
    /// to send the data.
    pub async fn recvmsg(&self, bufs: &mut [&mut [u8]], flags: RecvFlags) -> Result<(usize, A)> {
        self.receiver.recvmsg(bufs, flags).await
    }

    pub async fn write(&self, buf: &[u8]) -> Result<usize> {
        self.writev(&[buf]).await
    }

    pub async fn writev(&self, bufs: &[&[u8]]) -> Result<usize> {
        self.sendmsg(bufs, None, SendFlags::empty()).await
    }

    pub async fn sendmsg(
        &self,
        bufs: &[&[u8]],
        addr: Option<&A>,
        flags: SendFlags,
    ) -> Result<usize> {
        let state = self.state.read().unwrap();
        if addr.is_none() && !state.is_connected() {
            return_errno!(ENOTCONN, "the socket is not connected");
        }

        let res = if let Some(addr) = addr {
            drop(state);
            self.sender.sendmsg(bufs, addr, flags).await
        } else {
            let peer = self.common.peer_addr().unwrap();
            drop(state);
            self.sender.sendmsg(bufs, &peer, flags).await
        };

        let mut state = self.state.write().unwrap();
        if !state.is_bound() {
            state.mark_implicit_bind();
            // Start async recv after explicit binding or implicit binding
            self.receiver.initiate_async_recv();
        }

        res
    }

    pub fn poll(&self, mask: Events, poller: Option<&mut Poller>) -> Events {
        let pollee = self.common.pollee();
        pollee.poll(mask, poller)
    }

    pub fn register_observer(&self, observer: Arc<dyn Observer>, mask: Events) -> Result<()> {
        let pollee = self.common.pollee();
        pollee.register_observer(observer, mask);
        Ok(())
    }

    pub fn unregister_observer(&self, observer: &Arc<dyn Observer>) -> Result<Arc<dyn Observer>> {
        let pollee = self.common.pollee();
        pollee
            .unregister_observer(observer)
            .ok_or_else(|| errno!(ENOENT, "the observer is not registered"))
    }

    pub fn addr(&self) -> Result<A> {
        match self.common.addr() {
            Some(addr) => Ok(addr),
            None => {
                let state = self.state.read().unwrap();
                if state.is_bound() {
                    let addr = self.common.get_addr_from_host()?;
                    self.common.set_addr(&addr);
                    Ok(addr)
                } else {
                    Ok(A::default())
                }
            }
        }
    }

    pub fn peer_addr(&self) -> Result<A> {
        let state = self.state.read().unwrap();
        if !state.is_connected() {
            return_errno!(ENOTCONN, "the socket is not connected");
        }
        Ok(self.common.peer_addr().unwrap())
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
                // Datagram doesn't support listen
                cmd.set_output(0);
            },
            cmd: GetDomainCmd => {
                cmd.set_output(self.domain() as _);
            },
            cmd: GetPeerNameCmd => {
                let peer = self.peer_addr()?;
                cmd.set_output(AddrStorage(peer.to_c_storage()));
            },
            cmd: GetTypeCmd => {
                cmd.set_output(self.common.type_() as _);
            },
            _ => {
                return_errno!(EINVAL, "Not supported yet");
            }
        });
        Ok(())
    }

    fn cancel_requests(&self) {
        self.receiver.cancel_requests();
    }
}

impl<A: Addr + 'static, R: Runtime> Drop for DatagramSocket<A, R> {
    fn drop(&mut self) {
        self.common.set_closed();
        self.cancel_requests();
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for DatagramSocket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DatagramSocket")
            .field("common", &self.common)
            .field("state", &self.state.read().unwrap())
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

    pub fn is_explicit_bound(&self) -> bool {
        self.bind_state.is_explicit_bound()
    }

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

    pub fn is_explicit_bound(&self) -> bool {
        match self {
            Self::ExplicitBound => true,
            _ => false,
        }
    }

    pub fn is_implicit_bound(&self) -> bool {
        match self {
            Self::ImplicitBound => true,
            _ => false,
        }
    }
}
