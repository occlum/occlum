use prelude::*;
use std::{convert, error, fmt};

// TODO: remove errno.h

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Error {
    pub errno: Errno,
    pub desc: &'static str,
}

impl Error {
    pub fn new(errno: Errno, desc: &'static str) -> Error {
        let ret = Error { errno, desc };
        ret
    }
}

impl convert::From<(Errno, &'static str)> for Error {
    fn from(info: (Errno, &'static str)) -> Error {
        Error::new(info.0, info.1)
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        self.desc
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error: {} ({})", self.desc, self.errno)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum Errno {
    EUNDEF = 0,
    EPERM = 1,
    ENOENT = 2,
    ESRCH = 3,
    EINTR = 4,
    EIO = 5,
    ENXIO = 6,
    E2BIG = 7,
    ENOEXEC = 8,
    EBADF = 9,
    ECHILD = 10,
    EAGAIN = 11,
    ENOMEM = 12,
    EACCES = 13,
    EFAULT = 14,
    ENOTBLK = 15,
    EBUSY = 16,
    EEXIST = 17,
    EXDEV = 18,
    ENODEV = 19,
    ENOTDIR = 20,
    EISDIR = 21,
    EINVAL = 22,
    ENFILE = 23,
    EMFILE = 24,
    ENOTTY = 25,
    ETXTBSY = 26,
    EFBIG = 27,
    ENOSPC = 28,
    ESPIPE = 29,
    EROFS = 30,
    EMLINK = 31,
    EPIPE = 32,
    EDOM = 33,
    ERANGE = 34,
    EDEADLK = 35,
    ENAMETOOLONG = 36,
    ENOLCK = 37,
    ENOSYS = 38,
    ENOTEMPTY = 39,
    ELOOP = 40,
    EWOULDBLOCK = 41,
    ENOMSG = 42,
    EIDRM = 43,
    ECHRNG = 44,
    EL2NSYNC = 45,
    EL3HLT = 46,
    EL3RST = 47,
    ELNRNG = 48,
    EUNATCH = 49,
    ENOCSI = 50,
    EL2HLT = 51,
    EBADE = 52,
    EBADR = 53,
    EXFULL = 54,
    ENOANO = 55,
    EBADRQC = 56,
    EBADSLT = 57,
    EDEADLOCK = 58,
    EBFONT = 59,
    ENOSTR = 60,
    ENODATA = 61,
    ETIME = 62,
    ENOSR = 63,
    ENONET = 64,
    ENOPKG = 65,
    EREMOTE = 66,
    ENOLINK = 67,
    EADV = 68,
    ESRMNT = 69,
    ECOMM = 70,
    EPROTO = 71,
    EMULTIHOP = 72,
    EDOTDOT = 73,
    EBADMSG = 74,
    EOVERFLOW = 75,
    ENOTUNIQ = 76,
    EBADFD = 77,
    EREMCHG = 78,
    ELIBACC = 79,
    ELIBBAD = 80,
    ELIBSCN = 81,
    ELIBMAX = 82,
    ELIBEXEC = 83,
    EILSEQ = 84,
    ERESTART = 85,
    ESTRPIPE = 86,
    EUSERS = 87,
    ENOTSOCK = 88,
    EDESTADDRREQ = 89,
    EMSGSIZE = 90,
    EPROTOTYPE = 91,
    ENOPROTOOPT = 92,
    EPROTONOSUPPORT = 93,
    ESOCKTNOSUPPORT = 94,
    EOPNOTSUPP = 95,
    EPFNOSUPPORT = 96,
    EAFNOSUPPORT = 97,
    EADDRINUSE = 98,
    EADDRNOTAVAIL = 99,
    ENETDOWN = 100,
    ENETUNREACH = 101,
    ENETRESET = 102,
    ECONNABORTED = 103,
    ECONNRESET = 104,
    ENOBUFS = 105,
    EISCONN = 106,
    ENOTCONN = 107,
    ESHUTDOWN = 108,
    ETOOMANYREFS = 109,
    ETIMEDOUT = 110,
    ECONNREFUSED = 111,
    EHOSTDOWN = 112,
    EHOSTUNREACH = 113,
    EALREADY = 114,
    EINPROGRESS = 115,
    ESTALE = 116,
    EUCLEAN = 117,
    ENOTNAM = 118,
    ENAVAIL = 119,
    EISNAM = 120,
    EREMOTEIO = 121,
    EDQUOT = 122,
    ENOMEDIUM = 123,
    EMEDIUMTYPE = 124,
    ECANCELED = 125,
    ENOKEY = 126,
    EKEYEXPIRED = 127,
    EKEYREVOKED = 128,
    EKEYREJECTED = 129,
    EOWNERDEAD = 130,
    ENOTRECOVERABLE = 131,
    ERFKILL = 132,
    EHWPOISON = 133,
}

impl Errno {
    pub fn as_retval(&self) -> i32 {
        -(*self as i32)
    }
    pub fn from_errno(mut errno: i32) -> Self {
        if errno < 0 || errno > 133 {
            errno = 0;
        }
        unsafe { core::mem::transmute(errno as u8) }
    }
}

impl fmt::Display for Errno {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "errno = {}, \"{}\"",
            *self as u32,
            match *self {
                Errno::EPERM => "Operation not permitted",
                Errno::ENOENT => "No such file or directory",
                Errno::ESRCH => "No such process",
                Errno::EINTR => "Interrupted system call",
                Errno::EIO => "I/O error",
                Errno::ENXIO => "No such device or address",
                Errno::E2BIG => "Argument list too long",
                Errno::ENOEXEC => "Exec format error",
                Errno::EBADF => "Bad file number",
                Errno::ECHILD => "No child processes",
                Errno::EAGAIN => "Try again",
                Errno::ENOMEM => "Out of memory",
                Errno::EACCES => "Permission denied",
                Errno::EFAULT => "Bad address",
                Errno::ENOTBLK => "Block device required",
                Errno::EBUSY => "Device or resource busy",
                Errno::EEXIST => "File exists",
                Errno::EXDEV => "Cross-device link",
                Errno::ENODEV => "No such device",
                Errno::ENOTDIR => "Not a directory",
                Errno::EISDIR => "Is a directory",
                Errno::EINVAL => "Invalid argument",
                Errno::ENFILE => "File table overflow",
                Errno::EMFILE => "Too many open files",
                Errno::ENOTTY => "Not a typewriter",
                Errno::ETXTBSY => "Text file busy",
                Errno::EFBIG => "File too large",
                Errno::ENOSPC => "No space left on device",
                Errno::ESPIPE => "Illegal seek",
                Errno::EROFS => "Read-only file system",
                Errno::EMLINK => "Too many links",
                Errno::EPIPE => "Broken pipe",
                Errno::EDOM => "Math argument out of domain of func",
                Errno::ERANGE => "Math result not representable",
                Errno::EDEADLK => "Resource deadlock would occur",
                Errno::ENAMETOOLONG => "File name too long",
                Errno::ENOLCK => "No record locks available",
                Errno::ENOSYS => "Function not implemented",
                Errno::ENOTEMPTY => "Directory not empty",
                _ => "Unknown error",
            },
        )
    }
}
