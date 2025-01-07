use core::time::Duration;

use super::*;
use crate::fs::IoEvents as Events;
use crate::fs::{GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, IoctlCmd};
use crate::net::socket::NetlinkFamily;
use crate::{
    events::{Observer, Poller},
    fs::{IoNotifier, StatusFlags},
    match_ioctl_cmd_mut,
    net::socket::MsgFlags,
};
use core::ops::{Range, RangeFrom};

pub struct NetlinkSocket<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    sender: Arc<Sender<A, R>>,
    receiver: Arc<Receiver<A, R>>,
}

impl<A: Addr, R: Runtime> NetlinkSocket<A, R> {
    pub fn new(
        socket_type: SocketType,
        netlink_family: NetlinkFamily,
        nonblocking: bool,
    ) -> Result<Self> {
        let common = Arc::new(Common::new(
            socket_type,
            nonblocking,
            Some(netlink_family as i32),
        )?);
        let sender = Sender::new(common.clone());
        let receiver = Receiver::new(common.clone());

        // Start async recv
        receiver.initiate_async_recv();

        Ok(Self {
            common,
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

    pub fn type_(&self) -> SocketType {
        self.common.type_()
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

    pub fn connect(&self, peer_addr: Option<&A>) -> Result<()> {
        if let Some(peer) = peer_addr {
            self.common.set_peer_addr(peer);
        } else {
            self.common.reset_peer_addr();
        }
        Ok(())
    }

    pub fn close(&self) -> Result<()> {
        self.sender.shutdown();
        self.receiver.shutdown();
        self.common.set_closed();
        self.cancel_requests();
        Ok(())
    }

    pub fn shutdown(&self, how: Shutdown) -> Result<()> {
        return_errno!(EOPNOTSUPP, "Netlink does not support shutdown");
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
        let res = if addr.is_some() {
            self.sender.sendmsg(bufs, addr.unwrap(), flags, None)
        } else {
            let connected_addr = self.common.peer_addr();
            let peer_addr = if let Some(addr) = connected_addr {
                addr
            } else {
                // Sometimes Netlink has no assigning address in sendmsg and
                // its status is not connected. In this situation, netlink addr
                // use default dst_pid(0) and dst_groups(0).
                A::default()
            };
            self.sender.sendmsg(bufs, &peer_addr, flags, None)
        };

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
        Ok(self.common.peer_addr().unwrap_or_default())
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

impl<A: Addr + 'static, R: Runtime> Drop for NetlinkSocket<A, R> {
    fn drop(&mut self) {
        self.common.set_closed();
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for NetlinkSocket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetlinkSocket")
            .field("common", &self.common)
            .finish()
    }
}
