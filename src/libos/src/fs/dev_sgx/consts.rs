use super::*;

/// Ioctl to check if EDMM (Enclave Dynamic Memory Management) is supported
pub const SGX_CMD_NUM_IS_EDMM_SUPPORTED: u32 =
    StructuredIoctlNum::new::<i32>(0, SGX_MAGIC_CHAR, StructuredIoctlArgType::Output).as_u32();
/// Ioctl to get the EPID group ID
pub const SGX_CMD_NUM_GET_EPID_GROUP_ID: u32 = StructuredIoctlNum::new::<sgx_epid_group_id_t>(
    1,
    SGX_MAGIC_CHAR,
    StructuredIoctlArgType::Output,
)
.as_u32();
/// Ioctl to get quote
pub const SGX_CMD_NUM_GEN_QUOTE: u32 = StructuredIoctlNum::new::<IoctlGenQuoteArg>(
    2,
    SGX_MAGIC_CHAR,
    StructuredIoctlArgType::InputOutput,
)
.as_u32();

/// A magical number that distinguishes SGX ioctls for other ioctls
const SGX_MAGIC_CHAR: u8 = 's' as u8;
