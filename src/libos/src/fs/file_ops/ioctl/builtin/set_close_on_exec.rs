use super::*;

async_io::impl_ioctl_cmd! {
    pub struct SetCloseOnExec<Input=bool, Output=()> {}
}
