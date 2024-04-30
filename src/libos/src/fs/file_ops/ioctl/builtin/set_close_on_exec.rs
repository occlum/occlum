use super::*;

impl_ioctl_cmd! {
    pub struct SetCloseOnExec<Input=bool, Output=()> {}
}
