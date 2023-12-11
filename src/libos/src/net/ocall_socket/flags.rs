use super::*;

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
