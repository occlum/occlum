use sgx_trts::libc;

pub struct AddrStorage(pub (libc::sockaddr_storage, usize));
impl std::fmt::Debug for AddrStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AddrStorage")
            .field(&"sockaddr_storage")
            .field(&(self.0).1)
            .finish()
    }
}

crate::impl_ioctl_cmd! {
    pub struct GetPeerNameCmd<Input=(), Output=AddrStorage> {}
}
