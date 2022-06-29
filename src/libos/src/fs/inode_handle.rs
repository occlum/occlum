use super::*;

/// A Wrapper to handle the INode and the AsyncInode
pub enum InodeHandle {
    Sync(Arc<dyn INode>),
    Async(Arc<dyn AsyncInode>),
}

impl InodeHandle {
    pub fn from_sync(inode: Arc<dyn INode>) -> Self {
        Self::Sync(inode)
    }

    pub fn from_async(inode: Arc<dyn AsyncInode>) -> Self {
        Self::Async(inode)
    }

    pub fn as_sync(&self) -> Option<Arc<dyn INode>> {
        match self {
            Self::Sync(i) => Some(i.clone()),
            Self::Async(_) => None,
        }
    }

    pub fn as_async(&self) -> Option<Arc<dyn AsyncInode>> {
        match self {
            Self::Sync(_) => None,
            Self::Async(i) => Some(i.clone()),
        }
    }

    pub async fn metadata(&self) -> Result<Metadata> {
        match self {
            Self::Sync(i) => Ok(i.metadata()?),
            Self::Async(i) => Ok(i.metadata().await?),
        }
    }

    pub async fn set_metadata(&self, metadata: &Metadata) -> Result<()> {
        match self {
            Self::Sync(i) => Ok(i.set_metadata(metadata)?),
            Self::Async(i) => Ok(i.set_metadata(metadata).await?),
        }
    }

    pub fn allow_read(&self) -> bool {
        match self {
            Self::Sync(i) => i.allow_read().unwrap(),
            Self::Async(i) => true,
        }
    }

    pub fn allow_write(&self) -> bool {
        match self {
            Self::Sync(i) => i.allow_write().unwrap(),
            Self::Async(i) => true,
        }
    }

    pub async fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::Sync(i) => Ok(i.read_at(offset, buf)?),
            Self::Async(i) => Ok(i.read_at(offset, buf).await?),
        }
    }

    pub async fn write_at(&self, offset: usize, buf: &[u8]) -> Result<usize> {
        match self {
            Self::Sync(i) => Ok(i.write_at(offset, buf)?),
            Self::Async(i) => Ok(i.write_at(offset, buf).await?),
        }
    }

    pub async fn resize(&self, len: usize) -> Result<()> {
        match self {
            Self::Sync(i) => Ok(i.resize(len)?),
            Self::Async(i) => Ok(i.resize(len).await?),
        }
    }

    pub async fn create(&self, name: &str, type_: FileType, mode: u16) -> Result<Self> {
        match self {
            Self::Sync(i) => {
                let new_inode = i.create(name, type_, mode)?;
                Ok(Self::from_sync(new_inode))
            }
            Self::Async(i) => {
                let new_inode = i.create(name, type_, mode).await?;
                Ok(Self::from_async(new_inode))
            }
        }
    }

    pub async fn link(&self, name: &str, other: &Self) -> Result<()> {
        match self {
            Self::Sync(i) => {
                if other.as_sync().is_none() {
                    return_errno!(EXDEV, "not same fs");
                }
                Ok(i.link(name, other.as_sync().as_ref().unwrap())?)
            }
            Self::Async(i) => {
                if other.as_async().is_none() {
                    return_errno!(EXDEV, "not same fs");
                }
                Ok(i.link(name, other.as_async().as_ref().unwrap()).await?)
            }
        }
    }

    pub async fn unlink(&self, name: &str) -> Result<()> {
        match self {
            Self::Sync(i) => Ok(i.unlink(name)?),
            Self::Async(i) => Ok(i.unlink(name).await?),
        }
    }

    pub async fn read_link(&self) -> Result<String> {
        match self {
            Self::Sync(i) => {
                let mut content = vec![0u8; PATH_MAX];
                let len = i.read_at(0, &mut content)?;
                let path = std::str::from_utf8(&content[..len])
                    .map_err(|_| errno!(ENOENT, "invalid symlink content"))?;
                Ok(String::from(path))
            }
            Self::Async(i) => Ok(i.read_link().await?),
        }
    }

    pub async fn write_link(&self, target: &str) -> Result<()> {
        match self {
            Self::Sync(i) => {
                let data = target.as_bytes();
                i.write_at(0, data)?;
                Ok(())
            }
            Self::Async(i) => Ok(i.write_link(target).await?),
        }
    }

    pub async fn move_(&self, old_name: &str, target: &Self, new_name: &str) -> Result<()> {
        match self {
            Self::Sync(i) => {
                if target.as_sync().is_none() {
                    return_errno!(EXDEV, "not same fs");
                }
                Ok(i.move_(old_name, target.as_sync().as_ref().unwrap(), new_name)?)
            }
            Self::Async(i) => {
                if target.as_async().is_none() {
                    return_errno!(EXDEV, "not same fs");
                }
                Ok(
                    i.move_(old_name, target.as_async().as_ref().unwrap(), new_name)
                        .await?,
                )
            }
        }
    }

    pub async fn sync_all(&self) -> Result<()> {
        match self {
            Self::Sync(i) => Ok(i.sync_all()?),
            Self::Async(i) => Ok(i.sync_all().await?),
        }
    }

    pub async fn find(&self, name: &str) -> Result<Self> {
        match self {
            Self::Sync(i) => {
                let inode = i.find(name)?;
                Ok(Self::from_sync(inode))
            }
            Self::Async(i) => {
                let inode = i.find(name).await?;
                Ok(Self::from_async(inode))
            }
        }
    }

    pub async fn lookup_no_follow(&self, path: &str) -> Result<Self> {
        self.lookup(path, None).await
    }

    pub async fn lookup(&self, path: &str, max_follows: Option<usize>) -> Result<Self> {
        match self {
            Self::Sync(i) => {
                let inode = match max_follows {
                    Some(max_follows) => i.lookup_follow(path, max_follows)?,
                    None => i.lookup_follow(path, 0)?,
                };
                Ok(Self::from_sync(inode))
            }
            Self::Async(i) => {
                let inode = i.lookup_follow(path, max_follows).await?;
                Ok(Self::from_async(inode))
            }
        }
    }
}
