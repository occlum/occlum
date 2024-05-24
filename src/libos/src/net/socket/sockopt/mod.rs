mod get;
mod get_acceptconn;
mod get_domain;
mod get_error;
mod get_output;
mod get_peername;
mod get_sockbuf;
mod get_type;
mod set;
mod set_sockbuf;
mod timeout;

pub use get::{getsockopt_by_host, GetSockOptRawCmd};
pub use get_acceptconn::GetAcceptConnCmd;
pub use get_domain::GetDomainCmd;
pub use get_error::GetErrorCmd;
pub use get_output::*;
pub use get_peername::{AddrStorage, GetPeerNameCmd};
pub use get_sockbuf::{GetRecvBufSizeCmd, GetSendBufSizeCmd};
pub use get_type::GetTypeCmd;
pub use set::{setsockopt_by_host, SetSockOptRawCmd};
pub use set_sockbuf::{SetRecvBufSizeCmd, SetSendBufSizeCmd};
pub use timeout::{
    timeout_to_timeval, GetRecvTimeoutCmd, GetSendTimeoutCmd, SetRecvTimeoutCmd, SetSendTimeoutCmd,
};

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Clone, Copy, Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(i32)]
#[allow(non_camel_case_types)]
pub enum SockOptName {
    SO_DEBUG = 1,      // i32, bool, 0 or 1
    SO_REUSEADDR = 2,  // i32, bool, 0 or 1
    SO_TYPE = 3,       // [builtin] read-only, i32
    SO_ERROR = 4,      // read-only, i32
    SO_DONTROUTE = 5,  // i32, bool, 0 or 1, Equal to: send with MSG_DONTROUTE flag.
    SO_BROADCAST = 6,  // i32, bool, 0 or 1
    SO_SNDBUF = 7,     // i32
    SO_RCVBUF = 8,     // i32
    SO_KEEPALIVE = 9,  // i32, bool, 0 or 1
    SO_OOBINLINE = 10, // i32, bool, 0 or 1, Might equal to: recv with MSG_OOB flag.
    SO_NO_CHECK = 11,  // i32, bool, 0 or 1
    SO_PRIORITY = 12,  // i32, >= 0, <= 6
    SO_LINGER = 13,    // linger structure
    SO_BSDCOMPAT = 14, // removed in linux 2.2
    SO_REUSEPORT = 15, // i32, bool, 0 or 1
    SO_PASSCRED = 16,  // i32, bool, 0 or 1
    SO_PEERCRED = 17,  // read-only, ucred structure
    // TODO: there may be a bug.
    // select(2), poll(2), and epoll(7) indicate a socket as readable
    // only if at least SO_RCVLOWAT bytes are available.
    SO_RCVLOWAT = 18,                      // i32
    SO_SNDLOWAT = 19,                      // read-only, i32
    SO_RCVTIMEO_OLD = 20,                  // struct timeval
    SO_SNDTIMEO_OLD = 21,                  // struct timeval
    SO_SECURITY_AUTHENTICATION = 22,       // no doc / code
    SO_SECURITY_ENCRYPTION_TRANSPORT = 23, // no doc / code
    SO_SECURITY_ENCRYPTION_NETWORK = 24,   // no doc / code
    SO_BINDTODEVICE = 25,                  // array
    SO_ATTACH_FILTER = 26,                 // SO_GET_FILTER, BPF-related
    SO_DETACH_FILTER = 27,                 // SO_DETACH_BPF, BPF-related
    SO_PEERNAME = 28,                      // [builtin] read-only, peer name
    SO_TIMESTAMP_OLD = 29,                 // i32, bool, 0 or 1, cmsg-related
    SO_ACCEPTCONN = 30,                    // [builtin] read-only, i32, bool, 0 or 1
    SO_PEERSEC = 31,                       // read-only, array
    SO_SNDBUFFORCE = 32,                   // i32
    SO_RCVBUFFORCE = 33,                   // i32
    SO_PASSSEC = 34,                       // i32, bool, 0 or 1, cmsg-related
    SO_TIMESTAMPNS_OLD = 35,               // i32, bool, 0 or 1, cmsg-related
    SO_MARK = 36,                          // i32
    SO_TIMESTAMPING_OLD = 37,              // i32, bool, 0 or 1, cmsg-related
    SO_PROTOCOL = 38,                      // read-only, i32
    SO_DOMAIN = 39,                        // [builtin] read-only, i32
    SO_RXQ_OVFL = 40,                      // i32, bool, 0 or 1, cmsg-related
    SO_WIFI_STATUS = 41,                   // i32, bool, 0 or 1
    // TODO: there may be a bug when specify SO_PEEK_OFF and MSG_PEEK
    SO_PEEK_OFF = 42,                // i32
    SO_NOFCS = 43,                   // i32, bool, 0 or 1
    SO_LOCK_FILTER = 44,             // i32, bool, 0 or 1
    SO_SELECT_ERR_QUEUE = 45,        // i32, bool, 0 or 1
    SO_BUSY_POLL = 46,               // i32
    SO_MAX_PACING_RATE = 47,         // u64
    SO_BPF_EXTENSIONS = 48,          // BPF-related
    SO_INCOMING_CPU = 49,            // i32
    SO_ATTACH_BPF = 50,              // BPF-related
    SO_ATTACH_REUSEPORT_CBPF = 51,   // BPF-related
    SO_ATTACH_REUSEPORT_EBPF = 52,   // BPF-related
    SO_CNX_ADVICE = 53,              // write-only, i32
    SCM_TIMESTAMPING_OPT_STATS = 54, // no doc / code
    SO_MEMINFO = 55,                 // read-only, array
    SO_INCOMING_NAPI_ID = 56,        // read-only, i32
    SO_COOKIE = 57,                  // read-only, u64
    SCM_TIMESTAMPING_PKTINFO = 58,   // no doc / code
    SO_PEERGROUPS = 59,              // read-only, array
    SO_ZEROCOPY = 60,                // i32, bool, 0 or 1
    SO_TXTIME = 61,                  // SCM_TXTIME, struct sock_txtime
    SO_BINDTOIFINDEX = 62,           // i32
    SO_TIMESTAMP_NEW = 63,           // i32, bool, 0 or 1, cmsg-related
    SO_TIMESTAMPNS_NEW = 64,         // i32, bool, 0 or 1, cmsg-related
    SO_TIMESTAMPING_NEW = 65,        // i32, bool, 0 or 1, cmsg-related
    SO_RCVTIMEO_NEW = 66,            // struct timeval
    SO_SNDTIMEO_NEW = 67,            // struct timeval
    SO_DETACH_REUSEPORT_BPF = 68,    // BPF-related
}
