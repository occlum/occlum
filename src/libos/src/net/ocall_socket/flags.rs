use super::*;

bitflags! {
    pub struct SendFlags: i32 {
        const MSG_OOB          = 0x01;
        const MSG_DONTROUTE    = 0x04;
        const MSG_DONTWAIT     = 0x40;       // Nonblocking io
        const MSG_EOR          = 0x80;       // End of record
        const MSG_CONFIRM      = 0x0800;     // Confirm path validity
        const MSG_NOSIGNAL     = 0x4000;     // Do not generate SIGPIPE
        const MSG_MORE         = 0x8000;     // Sender will send more
    }
}

bitflags! {
    pub struct RecvFlags: i32 {
        const MSG_OOB          = 0x01;
        const MSG_PEEK         = 0x02;
        const MSG_TRUNC        = 0x20;
        const MSG_DONTWAIT     = 0x40;       // Nonblocking io
        const MSG_WAITALL      = 0x0100;     // Wait for a full request
        const MSG_ERRQUEUE     = 0x2000;     // Fetch message from error queue
        const MSG_CMSG_CLOEXEC = 0x40000000; // Set close_on_exec for file descriptor received through M_RIGHTS
    }
}

bitflags! {
    pub struct MsgHdrFlags: i32 {
        const MSG_OOB          = 0x01;
        const MSG_CTRUNC       = 0x08;
        const MSG_TRUNC        = 0x20;
        const MSG_EOR          = 0x80;       // End of record
        const MSG_ERRQUEUE     = 0x2000;     // Fetch message from error queue
        const MSG_NOTIFICATION = 0x8000;     // Only applicable to SCTP socket
    }
}

bitflags! {
    pub struct FileFlags: i32 {
        const SOCK_NONBLOCK = 0x800;
        const SOCK_CLOEXEC  = 0x80000;
    }
}
