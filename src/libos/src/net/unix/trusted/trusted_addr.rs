use super::untrusted::UNTRUSTED_SOCKS;
use super::*;
use crate::fs::{AsyncInodeExt, CreationFlags, EventFileFlags, FileMode, FileType, FsPath};
use crate::util::sync::*;
use std::any::Any;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::{cmp, mem, slice, str};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct TrustedAddr {
    unix_addr: UnixAddr,
    inode: Option<usize>, // If unix_addr is a real file name, there will be corresponding inode number
}

impl TrustedAddr {
    pub fn inner(&self) -> &UnixAddr {
        &self.unix_addr
    }

    pub fn inode(&self) -> Option<usize> {
        self.inode
    }

    pub fn as_str(&self) -> Result<&str> {
        self.unix_addr.get_path_name()
    }

    // Bind the unix address with the inode of the FS
    pub async fn bind_addr(&mut self) -> Result<()> {
        if let UnixAddr::Pathname(path) = &self.unix_addr {
            let inode_num = {
                let current = current!();
                let fs = current.fs();
                let file_ref = fs
                    .open_file(
                        &FsPath::try_from(path.as_ref())?,
                        (CreationFlags::O_CREAT | CreationFlags::O_EXCL).bits(),
                        FileMode::from_bits(0o777).unwrap(),
                    )
                    .await
                    .map_err(|e| {
                        if e.errno() == EEXIST {
                            errno!(EADDRINUSE)
                        } else {
                            e
                        }
                    })?;
                file_ref
                    .as_async_file_handle()
                    .unwrap()
                    .dentry()
                    .inode()
                    .metadata()
                    .await?
                    .inode
            };
            self.inode = Some(inode_num);
        }
        Ok(())
    }

    pub async fn bind_untrusted_addr(&mut self, host_addr: &UnixAddr) -> Result<()> {
        if let UnixAddr::Pathname(path) = &self.unix_addr {
            let (dir_inode, sock_name) = {
                let current = current!();
                let fs = current.fs();
                let path = FsPath::try_from(path.as_ref())?;
                if path.ends_with("/") {
                    return_errno!(EISDIR, "path is a directory");
                }
                fs.lookup_dirinode_and_basename(&path).await?
            };

            if !dir_inode.allow_write().await {
                return_errno!(EPERM, "libos socket file cannot be created");
            }

            let socket_inode = dir_inode
                .create(&sock_name, FileType::Socket, 0o0777)
                .await?;
            let data = host_addr.get_path_name()?.as_bytes();
            socket_inode.resize(data.len()).await?;
            socket_inode.write_at(0, data).await?;
        }
        Ok(())
    }

    // Return host OS FS path defined in Occlum.json and if it is a socket file (false: a name of dir)
    pub fn get_crossworld_sock_path(&self) -> Option<(UnixAddr, bool)> {
        let path_str = if let Ok(str) = self.as_str() {
            str
        } else {
            // unamed or abstract name address
            return None;
        };
        let untrusted_socks = UNTRUSTED_SOCKS.read().unwrap();
        let cross_world_socket_path = untrusted_socks.get(path_str);
        if let Some(socket_file_path) = cross_world_socket_path {
            return Some((socket_file_path.clone(), true));
        }

        // No file match, try dir match
        if let Some((libos_dir, host_dir)) = untrusted_socks
            .iter()
            .find(|(&libos_dir_path, _)| path_str.starts_with(libos_dir_path))
        {
            return Some((host_dir.clone(), false));
        }

        return None;
    }

    // Init inode field of TrustedAddr for connect.
    pub async fn try_init_inode(&mut self) -> Result<()> {
        if let UnixAddr::Pathname(path) = &self.unix_addr {
            let inode_num = {
                let current = current!();
                let fs = current.fs();
                let inode_file = fs.lookup_inode(&FsPath::try_from(path.as_ref())?).await?;
                inode_file.metadata().await?.inode
            };
            self.inode = Some(inode_num);
        }
        Ok(())
    }
}

impl Addr for TrustedAddr {
    fn domain() -> Domain {
        Domain::Unix
    }

    fn from_c_storage(c_addr: &libc::sockaddr_storage, c_addr_len: usize) -> Result<Self> {
        Ok(Self {
            unix_addr: UnixAddr::from_c_storage(c_addr, c_addr_len)?,
            inode: None,
        })
    }

    fn to_c_storage(&self) -> (libc::sockaddr_storage, usize) {
        self.unix_addr.to_c_storage()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_default(&self) -> bool {
        let trusted_addr_default = Self::default();
        *self == trusted_addr_default
    }
}
