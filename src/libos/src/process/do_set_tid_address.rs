use std::ptr::NonNull;

use crate::prelude::*;

pub fn do_set_tid_address(tidptr: *mut pid_t) -> Result<pid_t> {
    debug!("set_tid_address: tidptr: {:?}", tidptr);
    let clear_ctid = NonNull::new(tidptr);
    let current = current!();
    current.set_clear_ctid(clear_ctid);
    Ok(current.tid())
}
