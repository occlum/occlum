use super::*;
use async_io::socket::MsgFlags;
use async_io::socket::{NetlinkFamily, Type};
use byteorder::{ByteOrder, NativeEndian};
use core::ops::{Range, RangeFrom};

pub struct NetlinkSocket<A: Addr + 'static, R: Runtime> {
    common: Arc<Common<A, R>>,
    sender: Arc<Sender<A, R>>,
    receiver: Arc<Receiver<A, R>>,
}

impl<A: Addr, R: Runtime> NetlinkSocket<A, R> {
    pub fn new(
        socket_type: Type,
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

    pub fn addr(&self) -> Result<A> {
        let common = &self.common;

        // Always get addr from host.
        // Because for IP socket, users can specify "0" as port and the kernel should select a usable port for him.
        // Thus, when calling getsockname, this should be updated.
        let addr = common.get_addr_from_host()?;
        common.set_addr(&addr);
        Ok(addr)
    }

    pub fn peer_addr(&self) -> Result<A> {
        Ok(self.common.peer_addr().unwrap_or_default())
    }

    pub async fn connect(&self, peer_addr: Option<&A>) -> Result<()> {
        if let Some(peer) = peer_addr {
            self.common.set_peer_addr(peer);
        } else {
            self.common.reset_peer_addr();
        }
        Ok(())
    }

    pub fn bind(&self, addr: &A) -> Result<()> {
        do_bind(self.host_fd(), addr)?;

        self.common.set_addr(addr);
        Ok(())
    }

    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        self.readv(&mut [buf]).await
    }

    pub async fn readv(&self, bufs: &mut [&mut [u8]]) -> Result<usize> {
        self.recvmsg(bufs, RecvFlags::empty())
            .await
            .map(|(ret, ..)| ret)
    }

    pub async fn recvmsg(
        &self,
        bufs: &mut [&mut [u8]],
        flags: RecvFlags,
    ) -> Result<(usize, Option<A>, MsgFlags, usize)> {
        self.receiver.recvmsg(bufs, flags, None).await
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
        let res = if addr.is_some() {
            self.sender.sendmsg(bufs, addr.unwrap(), flags, None).await
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
            self.sender.sendmsg(bufs, &peer_addr, flags, None).await
        };

        res
    }

    pub fn shutdown(&self, _how: Shutdown) -> Result<()> {
        return_errno!(EOPNOTSUPP, "Netlink does not support shutdown");
    }

    pub async fn close(&self) -> Result<()> {
        self.sender.shutdown();
        self.receiver.shutdown();
        self.common.set_closed();
        self.cancel_requests().await;
        Ok(())
    }

    async fn cancel_requests(&self) {
        self.receiver.cancel_recv_requests().await;
        self.sender.try_clear_msg_queue_when_close().await;
    }

    pub fn poll(&self, mask: Events, poller: Option<&Poller>) -> Events {
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

    pub fn ioctl(&self, cmd: &mut dyn IoctlCmd) -> Result<()> {
        async_io::match_ioctl_cmd_mut!(&mut *cmd, {
            cmd: GetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
            },
            cmd: SetSockOptRawCmd => {
                cmd.execute(self.host_fd())?;
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
}

impl<A: Addr + 'static, R: Runtime> std::fmt::Debug for NetlinkSocket<A, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetlinkSocket")
            .field("common", &self.common)
            .finish()
    }
}

/// Represent a multi-bytes field with a fixed size in a packet
pub(crate) type Field = Range<usize>;
/// Represent a field that starts at a given index in a packet
pub(crate) type Rest = RangeFrom<usize>;

// Represent each field for a netlink msg
const LENGTH: Field = 0..4;
const MESSAGE_TYPE: Field = 4..6;
#[allow(unused)]
const FLAGS: Field = 6..8;
#[allow(unused)]
const SEQUENCE_NUMBER: Field = 8..12;
#[allow(unused)]
const PORT_NUMBER: Field = 12..16;
const PAYLOAD: Rest = 16..;

/// Length of a Netlink packet header
pub const NETLINK_HEADER_LEN: usize = PAYLOAD.start;

const NLMSG_MIN_TYPE: u16 = 0x10; /* < 0x10: reserved control messages */

pub struct NetlinkMsg<T> {
    buffer: T,
}

impl<T: AsRef<[u8]>> NetlinkMsg<T> {
    pub fn new(buffer: T) -> Option<NetlinkMsg<T>> {
        let msg = NetlinkMsg { buffer };

        // Reference: NLMSG_OK macro
        if msg.buffer.as_ref().len() >= NETLINK_HEADER_LEN
            && msg.length() as usize >= NETLINK_HEADER_LEN
            && msg.length() as usize <= msg.buffer.as_ref().len()
            && msg.message_type() >= NLMSG_MIN_TYPE
        {
            Some(msg)
        } else {
            None
        }
    }

    #[allow(unused)]
    pub fn into_inner(self) -> T {
        self.buffer
    }

    pub fn length(&self) -> u32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u32(&data[LENGTH])
    }

    pub fn message_type(&self) -> u16 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u16(&data[MESSAGE_TYPE])
    }

    #[allow(unused)]
    pub fn flags(&self) -> u16 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u16(&data[FLAGS])
    }

    #[allow(unused)]
    pub fn sequence_number(&self) -> u32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u32(&data[SEQUENCE_NUMBER])
    }

    #[allow(unused)]
    pub fn port_number(&self) -> u32 {
        let data = self.buffer.as_ref();
        NativeEndian::read_u32(&data[PORT_NUMBER])
    }
}
