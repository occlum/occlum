//! Non-builtin ioctls.

use super::*;

#[derive(Debug)]
pub struct NonBuiltinIoctlCmd<'a> {
    cmd_num: StructuredIoctlNum,
    arg_buf: Option<&'a mut [u8]>,
}

impl<'a> NonBuiltinIoctlCmd<'a> {
    pub unsafe fn new(
        cmd_num: StructuredIoctlNum,
        arg_ptr: *mut u8,
    ) -> Result<NonBuiltinIoctlCmd<'a>> {
        let arg_buf = if cmd_num.require_arg() {
            if arg_ptr.is_null() {
                return_errno!(EINVAL, "arg_ptr must be provided for the ioctl");
            }
            let arg_size = cmd_num.arg_size();
            let arg_slice = unsafe { std::slice::from_raw_parts_mut::<'a>(arg_ptr, arg_size) };
            Some(arg_slice)
        } else {
            None
        };
        Ok(NonBuiltinIoctlCmd { cmd_num, arg_buf })
    }

    pub fn cmd_num(&self) -> &StructuredIoctlNum {
        &self.cmd_num
    }

    pub fn arg<T>(&self) -> Result<&T> {
        if self.cmd_num.arg_type().can_be_input() == false {
            return_errno!(EINVAL, "cannot get a constant argument");
        }
        if std::mem::size_of::<T>() != self.cmd_num.arg_size() {
            return_errno!(
                EINVAL,
                "the size of target type does not match the given buf size"
            );
        }

        let arg_ref = unsafe { &*(self.arg_buf.as_ref().unwrap().as_ptr() as *const T) };
        Ok(arg_ref)
    }

    pub fn arg_mut<T>(&mut self) -> Result<&mut T> {
        if self.cmd_num.arg_type().can_be_output() == false {
            return_errno!(EINVAL, "cannot get a mutable argument");
        }
        if std::mem::size_of::<T>() != self.cmd_num.arg_size() {
            return_errno!(
                EINVAL,
                "the size of target type does not match the given buf size"
            );
        }

        let arg_mut = unsafe { &mut *(self.arg_buf.as_mut().unwrap().as_mut_ptr() as *mut T) };
        Ok(arg_mut)
    }

    pub fn arg_ptr(&self) -> *const u8 {
        self.arg_buf
            .as_ref()
            .map_or(std::ptr::null(), |arg_slice| arg_slice.as_ptr())
    }

    pub fn arg_len(&self) -> usize {
        self.cmd_num.arg_size()
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct StructuredIoctlNum {
    cmd_id: u8,
    magic_char: u8,
    arg_size: u16,
    arg_type: StructuredIoctlArgType,
}

impl StructuredIoctlNum {
    pub const fn new<T>(
        cmd_id: u8,
        magic_char: u8,
        arg_type: StructuredIoctlArgType,
    ) -> StructuredIoctlNum {
        // TODO: make sure the size of T is not too big
        // assert!(std::mem::size_of::<T>() <= (std::u16::MAX as usize));
        let arg_size = std::mem::size_of::<T>() as u16;
        StructuredIoctlNum {
            cmd_id,
            magic_char,
            arg_size,
            arg_type,
        }
    }

    pub fn from_u32(raw_cmd_num: u32) -> Result<StructuredIoctlNum> {
        // bits: [0, 8)
        let cmd_id = (raw_cmd_num >> 0) as u8;
        // bits: [8, 16)
        let magic_char = (raw_cmd_num >> 8) as u8;
        // bits: [16, 30)
        let arg_size = ((raw_cmd_num >> 16) as u16) & 0x3FFF_u16;
        // bits: [30, 32)
        let arg_type = {
            let type_bits = ((raw_cmd_num) >> 30) as u8;
            StructuredIoctlArgType::from_u8(type_bits)
        };

        if arg_type == StructuredIoctlArgType::Void {
            if arg_size != 0 {
                return_errno!(EINVAL, "invalid combination between type and size");
            }
        } else {
            if arg_size == 0 {
                return_errno!(EINVAL, "invalid combination between type and size");
            }
        }

        Ok(StructuredIoctlNum {
            cmd_id,
            magic_char,
            arg_size,
            arg_type,
        })
    }

    pub const fn as_u32(&self) -> u32 {
        (self.cmd_id as u32)
            | (self.magic_char as u32) << 8
            | (self.arg_size as u32) << 16
            | (self.arg_type as u32) << 30
    }

    pub fn require_arg(&self) -> bool {
        self.arg_type != StructuredIoctlArgType::Void
    }

    pub fn cmd_id(&self) -> u8 {
        self.cmd_id
    }

    pub fn magic_char(&self) -> u8 {
        self.magic_char
    }

    pub fn arg_size(&self) -> usize {
        self.arg_size as usize
    }

    pub fn arg_type(&self) -> StructuredIoctlArgType {
        self.arg_type
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u8)]
pub enum StructuredIoctlArgType {
    Void = 0,
    Input = 1,
    Output = 2,
    InputOutput = 3,
}

impl StructuredIoctlArgType {
    pub fn from_u8(type_bits: u8) -> StructuredIoctlArgType {
        if type_bits > StructuredIoctlArgType::InputOutput as u8 {
            panic!("invalid bits for StructuredIoctlArgType");
        }
        unsafe { core::mem::transmute(type_bits) }
    }

    pub fn can_be_input(&self) -> bool {
        *self == StructuredIoctlArgType::Input || *self == StructuredIoctlArgType::InputOutput
    }

    pub fn can_be_output(&self) -> bool {
        *self == StructuredIoctlArgType::Output || *self == StructuredIoctlArgType::InputOutput
    }
}
