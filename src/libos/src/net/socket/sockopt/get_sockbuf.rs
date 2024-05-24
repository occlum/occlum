crate::impl_ioctl_cmd! {
    pub struct GetSendBufSizeCmd<Input=(), Output=usize> {}
}

crate::impl_ioctl_cmd! {
    pub struct GetRecvBufSizeCmd<Input=(), Output=usize> {}
}
