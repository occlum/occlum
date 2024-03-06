crate::impl_ioctl_cmd! {
    pub struct GetSndBufSizeCmd<Input=(), Output=usize> {}
}

crate::impl_ioctl_cmd! {
    pub struct GetRcvBufSizeCmd<Input=(), Output=usize> {}
}
