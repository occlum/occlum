use bitflags::bitflags;
use sgx_trts::libc;
use std::ffi::c_uint;

// Flags to use when sending data through a socket
bitflags! {
    pub struct SendFlags: i32 {
        const MSG_OOB          = 0x01;      // Sends out-of-band data on sockets
        const MSG_DONTROUTE    = 0x04;      // Don't use a gateway to send out the packet
        const MSG_DONTWAIT     = 0x40;      // Nonblocking io
        const MSG_EOR          = 0x80;      // End of record
        const MSG_CONFIRM      = 0x0800;    // Confirm path validity
        const MSG_NOSIGNAL     = 0x4000;    // Do not generate SIGPIPE
        const MSG_MORE         = 0x8000;    // Sender will send more
    }
}

// Flags to use when receiving data through a socket
bitflags! {
    pub struct RecvFlags: i32 {
        const MSG_OOB          = 0x01;          // Recv out-of-band data
        const MSG_PEEK         = 0x02;          // Return data without removing that
        const MSG_TRUNC        = 0x20;          // Return the real length of the packet or datagram
        const MSG_DONTWAIT     = 0x40;          // Nonblocking io
        const MSG_WAITALL      = 0x0100;        // Wait for a full request
        const MSG_ERRQUEUE     = 0x2000;        // Fetch message from error queue
        // recvmsg only
        const MSG_CMSG_CLOEXEC = 0x40000000;    // Set close_on_exec for file descriptor received through SCM_RIGHTS
    }
}

bitflags! {
    pub struct MsgFlags: i32 {
        const MSG_OOB          = 0x01;      // Expedited or out-of-band data was received
        const MSG_CTRUNC       = 0x08;      // Some control data was discarded
        const MSG_TRUNC        = 0x20;      // The trailing portion of a datagram was discarded
        const MSG_EOR          = 0x80;      // End of record
        const MSG_ERRQUEUE     = 0x2000;    // Fetch message from error queue
        const MSG_NOTIFICATION = 0x8000;     // Only applicable to SCTP socket
    }
}

// Flags to use when creating a new socket
bitflags! {
    pub struct SocketFlags: i32 {
        const SOCK_NONBLOCK = 0x800;
        const SOCK_CLOEXEC  = 0x80000;
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct mmsghdr {
    pub msg_hdr: libc::msghdr,
    pub msg_len: c_uint,
}
