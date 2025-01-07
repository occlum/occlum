use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

use super::*;
use crate::fs::IoEvents as Events;
use crate::fs::{GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, IoctlCmd};
use crate::{
    events::{Observer, Poller},
    fs::{IoNotifier, StatusFlags},
    match_ioctl_cmd_mut,
    net::socket::MsgFlags,
    net::socket::{IPProtocol, SocketProtocol},
};

pub struct IpPacket<A: Addr + 'static, R: Runtime> {
    proto: ProtocolType,
    is_connected: AtomicBool,
    common: Arc<Common<A, R>>,
    sender: Arc<Sender<A, R>>,
    receiver: Arc<Receiver<A, R>>,
}

impl<A: Addr, R: Runtime> IpPacket<A, R> {
    pub fn new(nonblocking: bool, proto: IPProtocol) -> Result<Self> {
        let common = Arc::new(Common::new(
            SocketType::RAW,
            nonblocking,
            Some(proto as i32),
        )?);
        let is_connected = AtomicBool::new(false);
        let proto = match proto {
            IPProtocol::IPPROTO_TCP => ProtocolType::TCP,
            IPProtocol::IPPROTO_UDP => ProtocolType::UDP,
            IPProtocol::IPPROTO_RAW => ProtocolType::RAW,
            IPProtocol::IPPROTO_IP => ProtocolType::IP,
            _ => ProtocolType::Other,
        };
        let sender = Sender::new(common.clone());
        let receiver = Receiver::new(common.clone());
        receiver.initiate_async_recv();
        Ok(Self {
            common,
            is_connected,
            proto,
            sender,
            receiver,
        })
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

    pub fn bind(&self, addr: &A) -> Result<()> {
        do_bind(self.host_fd(), addr)?;
        self.common.set_addr(addr);

        Ok(())
    }

    // ip packet socket don't support getpeername, and if we connect address is unspec,
    // we mark it disconnected
    pub fn connect(&self, peer_addr: Option<&A>) -> Result<()> {
        if let Some(peer) = peer_addr {
            do_connect(self.host_fd(), Some(peer))?;

            self.receiver.reset_shutdown();
            self.sender.reset_shutdown();
            self.common.set_peer_addr(peer);

            self.is_connected.store(true, Ordering::Release);
        } else {
            do_connect::<A>(self.host_fd(), None)?;
            self.common.reset_peer_addr();
            self.is_connected.store(false, Ordering::Release);
        }
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.readv(&mut [buf])
    }

    pub fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.recvmsg(bufs, RecvFlags::empty(), None)
            .map(|(ret, ..)| ret)
    }

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
        if self.proto == ProtocolType::RAW {
            // SOCK_RAW+IPPROTO_RAW sockets need to include ip header
            let buf_len: usize = bufs.iter().map(|buf| buf.len()).sum();
            let hdr_len = match A::domain() {
                Domain::INET => IPV4HDR_SIZE,
                Domain::INET6 => IPV6HDR_SIZE,
                _ => return_errno!(EINVAL, "ip packet socket domain not support"),
            };
            if buf_len < hdr_len {
                return_errno!(EINVAL, "raw socket writes too small")
            }
        }

        let is_connected = self.is_connected.load(Ordering::Acquire);
        // if addr is none and ip raw socket don't connected, sendmsg() fails
        if addr.is_none() && !is_connected {
            return_errno!(EDESTADDRREQ, "Destination address required");
        }

        let res = if addr.is_some() {
            self.sender.sendmsg(bufs, addr.unwrap(), flags, None)
        } else {
            let peer_addr = self.common.peer_addr();
            if let Some(peer) = peer_addr.as_ref() {
                self.sender.sendmsg(bufs, peer, flags, control)
            } else {
                return_errno!(EDESTADDRREQ, "Destination address required");
            }
        };
        res
    }

    pub fn close(&self) -> Result<()> {
        self.sender.shutdown();
        self.receiver.shutdown();
        self.common.set_closed();
        self.cancel_requests();
        Ok(())
    }

    fn cancel_requests(&self) {
        self.receiver.cancel_recv_requests();
        self.sender.try_clear_msg_queue_when_close();
    }

    // Return error when unconnected,
    // and shutdown is a no-op for raw sockets.
    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        if !self.is_connected.load(Ordering::Acquire) {
            return_errno!(ENOTCONN, "The ip raw socket is not connected");
        }
        Ok(())
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
        let is_connected = self.is_connected.load(Ordering::Acquire);
        if !is_connected {
            return_errno!(ENOTCONN, "The ip raw socket isn't connected")
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
                // IP packet socket doesn't support listen
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
                return_errno!(EOPNOTSUPP, "packet socket doesn't support connect")
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
}

impl<A: Addr + 'static, R: Runtime> Drop for IpPacket<A, R> {
    fn drop(&mut self) {
        self.common.set_closed();
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for IpPacket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpPacket")
            .field("common", &self.common)
            .finish()
    }
}

#[derive(PartialEq, Clone, Copy, Eq, Debug)]
enum ProtocolType {
    RAW,
    TCP,
    UDP,
    IP,
    Other,
}
