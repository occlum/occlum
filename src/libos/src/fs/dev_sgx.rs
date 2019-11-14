use super::*;

#[derive(Debug)]
pub struct DevSgx;

const SGX_MAGIC_CHAR: u8 = 's' as u8;

/// Ioctl to check if EDMM (Enclave Dynamic Memory Management) is supported
const SGX_CMD_NUM_IS_EDMM_SUPPORTED: u32 =
    StructuredIoctlNum::new::<i32>(0, SGX_MAGIC_CHAR, StructuredIoctlArgType::Output).as_u32();

impl File for DevSgx {
    fn ioctl(&self, cmd: &mut IoctlCmd) -> Result<()> {
        let nonbuiltin_cmd = match cmd {
            IoctlCmd::NonBuiltin(nonbuiltin_cmd) => nonbuiltin_cmd,
            _ => return_errno!(EINVAL, "unknown ioctl cmd for /dev/sgx"),
        };
        let cmd_num = nonbuiltin_cmd.cmd_num().as_u32();
        match cmd_num {
            SGX_CMD_NUM_IS_EDMM_SUPPORTED => {
                let arg = nonbuiltin_cmd.arg_mut::<i32>()?;
                *arg = 0; // no support for now
            }
            _ => {
                return_errno!(EINVAL, "unknown ioctl cmd for /dev/sgx");
            }
        }
        Ok(())
    }

    fn as_any(&self) -> &Any {
        self
    }
}
