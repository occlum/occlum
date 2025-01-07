use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

use super::*;
use crate::fs::{GetIfConf, GetIfReqWithRawCmd, GetReadBufLen, IoctlCmd};
use crate::fs::{IoEvents as Events, SetNonBlocking};
use crate::net::socket::EthernetProtocol;

use crate::{
    events::{Observer, Poller},
    fs::{IoNotifier, StatusFlags},
    match_ioctl_cmd_mut,
    net::socket::MsgFlags,
};

pub struct RawPacket<A: Addr + 'static, R: Runtime> {
    is_bound: AtomicBool,
    common: Arc<Common<A, R>>,
    sender: Arc<Sender<A, R>>,
    receiver: Arc<Receiver<A, R>>,
}

impl<A: Addr, R: Runtime> RawPacket<A, R> {
    pub fn new(
        socket_type: SocketType,
        protocol: EthernetProtocol,
        nonblocking: bool,
    ) -> Result<Self> {
        // convert to big endian to create host's socket
        let ethernet_proto = EthernetProtocol::to_network_byte_order(protocol);
        let common = Arc::new(Common::new(
            socket_type,
            nonblocking,
            Some(ethernet_proto as i32),
        )?);
        let sender = Sender::new(common.clone());
        let receiver = Receiver::new(common.clone());
        let is_bound = AtomicBool::new(false);
        receiver.initiate_async_recv();
        Ok(Self {
            is_bound,
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

    // packet socket can double bind
    pub fn bind(&self, addr: &A) -> Result<()> {
        do_bind(self.host_fd(), addr)?;

        self.common.set_addr(addr);
        self.is_bound.store(true, Ordering::Release);
        self.receiver.initiate_async_recv();
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

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
        let pollee = self.common.pollee();
        pollee.poll(mask, poller)
    }

    pub fn addr(&self) -> Result<A> {
        let common = &self.common;

        // Always get addr from host.
        let addr = common.get_addr_from_host()?;
        common.set_addr(&addr);
        Ok(addr)
    }

    pub fn notifier(&self) -> &IoNotifier {
        let notifier = self.common.notifier();
        notifier
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
                // don't support peername
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
            // if addr is None, but packet socket is bound, then we can try send
            if self.is_bound.load(Ordering::Acquire) {
                let addr = self.common.addr().unwrap();
                self.sender.sendmsg(bufs, &addr, flags, None)
            } else {
                return_errno!(ENXIO, "packet need to bind before send")
            }
        };

        res
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

impl<A: Addr + 'static, R: Runtime> Drop for RawPacket<A, R> {
    fn drop(&mut self) {
        self.common.set_closed();
    }
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for RawPacket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawPacket")
            .field("common", &self.common)
            .finish()
    }
}
