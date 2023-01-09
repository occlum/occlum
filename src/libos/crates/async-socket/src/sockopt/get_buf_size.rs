async_io::impl_ioctl_cmd! {
    pub struct GetSndBufSizeCmd<Input=(), Output=usize> {}
}

async_io::impl_ioctl_cmd! {
    pub struct GetRcvBufSizeCmd<Input=(), Output=usize> {}
}
