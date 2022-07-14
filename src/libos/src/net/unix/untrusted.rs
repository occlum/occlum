use super::*;

use crate::config::LIBOS_CONFIG;
use crate::prelude::*;
use std::collections::HashMap;
use std::env;
use std::path::Path;

lazy_static! {
    pub static ref UNTRUSTED_SOCKS: RwLock<HashMap<&'static str, UnixAddr>> = RwLock::new(HashMap::new()); // libos path -> host path
}

pub fn untrusted_unix_socks_init() {
    let mut untrusted_socks = UNTRUSTED_SOCKS.write().unwrap();
    if let Some(socks) = &LIBOS_CONFIG.untrusted_unix_socks {
        socks.iter().for_each(|sock| {
            let libos_addr = sock.libos.as_path().to_str().expect("path is not valid");
            let host_str = sock.host.as_path();
            let absolute_host_path = if host_str.is_absolute() {
                host_str.to_path_buf()
            } else {
                env::current_dir().unwrap().join(host_str)
            };
            let host_unix_addr = UnixAddr::new_with_path_name(absolute_host_path.to_str().unwrap());
            untrusted_socks.insert(libos_addr, host_unix_addr);
        })
    }
}
