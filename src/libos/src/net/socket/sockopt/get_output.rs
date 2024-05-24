use super::{GetRecvTimeoutCmd, GetSendTimeoutCmd};

use super::{
    GetAcceptConnCmd, GetDomainCmd, GetErrorCmd, GetPeerNameCmd, GetRecvBufSizeCmd,
    GetSendBufSizeCmd, GetSockOptRawCmd, GetTypeCmd,
};

use libc::timeval;

use crate::prelude::*;

pub trait GetOutputAsBytes {
    fn get_output_as_bytes(&self) -> Option<&[u8]>;
}

impl GetOutputAsBytes for GetSockOptRawCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output()
    }
}

impl GetOutputAsBytes for GetDomainCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(val_ref as *const _ as *const u8, std::mem::size_of::<i32>())
        })
    }
}

impl GetOutputAsBytes for GetErrorCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(val_ref as *const _ as *const u8, std::mem::size_of::<i32>())
        })
    }
}

impl GetOutputAsBytes for GetAcceptConnCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(val_ref as *const _ as *const u8, std::mem::size_of::<i32>())
        })
    }
}

impl GetOutputAsBytes for GetPeerNameCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(&(val_ref.0).0 as *const _ as *const u8, (val_ref.0).1)
        })
    }
}

impl GetOutputAsBytes for GetTypeCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(val_ref as *const _ as *const u8, std::mem::size_of::<i32>())
        })
    }
}

impl GetOutputAsBytes for GetSendBufSizeCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(
                val_ref as *const _ as *const u8,
                std::mem::size_of::<usize>(),
            )
        })
    }
}

impl GetOutputAsBytes for GetRecvBufSizeCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(
                val_ref as *const _ as *const u8,
                std::mem::size_of::<usize>(),
            )
        })
    }
}

impl GetOutputAsBytes for GetRecvTimeoutCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(
                val_ref as *const _ as *const u8,
                std::mem::size_of::<timeval>(),
            )
        })
    }
}

impl GetOutputAsBytes for GetSendTimeoutCmd {
    fn get_output_as_bytes(&self) -> Option<&[u8]> {
        self.output().map(|val_ref| unsafe {
            std::slice::from_raw_parts(
                val_ref as *const _ as *const u8,
                std::mem::size_of::<timeval>(),
            )
        })
    }
}
