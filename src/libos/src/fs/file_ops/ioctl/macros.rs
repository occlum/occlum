//! Macros to implement `BuiltinIoctlNum` and `IoctlRawCmd` given a list of ioctl
//! names, numbers, and argument types.

/// Implement `BuiltinIoctlNum` and `IoctlRawCmd`.
#[macro_export]
macro_rules! impl_ioctl_nums_and_cmds {
    ($( $ioctl_name: ident => ( $ioctl_num: expr, $($ioctl_type_tt: tt)* ) ),+,) => {
        // Implement BuiltinIoctlNum given ioctl names and their numbers
        impl_builtin_ioctl_nums! {
            $(
                $ioctl_name => ( $ioctl_num, has_arg!($($ioctl_type_tt)*) ),
            )*
        }

        // Implement IoctlRawCmd given ioctl names and their argument types
        impl_ioctl_cmds! {
            $(
                $ioctl_name => ( $($ioctl_type_tt)*),
            )*
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// BuiltinIoctlNum
////////////////////////////////////////////////////////////////////////////////

macro_rules! impl_builtin_ioctl_nums {
    ($($ioctl_name: ident => ($ioctl_num: expr, $ioctl_has_arg: expr)),+,) => {
        #[derive(Debug, Copy, Clone, PartialEq)]
        pub enum BuiltinIoctlNum {
            $(
                $ioctl_name = $ioctl_num,
            )*
        }

        impl BuiltinIoctlNum {
            pub fn from_u32(raw_cmd_num: u32) -> Option<BuiltinIoctlNum> {
                let cmd_num = match raw_cmd_num {
                    $(
                        $ioctl_num => BuiltinIoctlNum::$ioctl_name,
                    )*
                    _ => return None,
                };
                Some(cmd_num)
            }

            pub fn require_arg(&self) -> bool {
                match self {
                    $(
                        BuiltinIoctlNum::$ioctl_name => $ioctl_has_arg,
                    )*
                }
            }
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// IoctlRawCmd
////////////////////////////////////////////////////////////////////////////////

macro_rules! impl_ioctl_cmds {
    ($( $ioctl_name: ident => ( $($ioctl_type_tt: tt)* ) ),+,) => {
        #[derive(Debug)]
        pub enum IoctlRawCmd<'a> {
            $(
                $ioctl_name( get_arg_type!($($ioctl_type_tt)*) ),
            )*
            NonBuiltin(NonBuiltinIoctlCmd<'a>),
        }

        impl<'a> IoctlRawCmd<'a> {
            pub unsafe fn new(cmd_num: u32, arg_ptr: *mut u8) -> Result<IoctlRawCmd<'a>> {
                if let Some(builtin_cmd_num) = BuiltinIoctlNum::from_u32(cmd_num) {
                    unsafe { Self::new_builtin_cmd(builtin_cmd_num, arg_ptr) }
                } else {
                    unsafe { Self::new_nonbuiltin_cmd(cmd_num, arg_ptr) }
                }
            }

            unsafe fn new_builtin_cmd(cmd_num: BuiltinIoctlNum, arg_ptr: *mut u8) -> Result<IoctlRawCmd<'a>> {
                if cmd_num.require_arg() && arg_ptr.is_null() {
                    return_errno!(EINVAL, "arg_ptr cannot be null");
                }
                // Note that we do allow the caller to give an non-null arg even
                // when the ioctl cmd does not take an argument

                let cmd = match cmd_num {
                    $(
                        BuiltinIoctlNum::$ioctl_name => {
                            let arg = get_arg!($($ioctl_type_tt)*, arg_ptr);
                            IoctlRawCmd::$ioctl_name(arg)
                        }
                    )*
                };
                Ok(cmd)
            }

            unsafe fn new_nonbuiltin_cmd(cmd_num: u32, arg_ptr: *mut u8) -> Result<IoctlRawCmd<'a>> {
                let structured_cmd_num = StructuredIoctlNum::from_u32(cmd_num)?;
                let inner_cmd = unsafe { NonBuiltinIoctlCmd::new(structured_cmd_num, arg_ptr)? };
                Ok(IoctlRawCmd::NonBuiltin(inner_cmd))
            }

            pub fn arg_ptr(&self) -> *const u8 {
                match self {
                    $(
                        IoctlRawCmd::$ioctl_name(arg_ref) => get_arg_ptr!($($ioctl_type_tt)*, arg_ref),
                    )*
                    IoctlRawCmd::NonBuiltin(inner) => inner.arg_ptr(),
                }
            }

            pub fn arg_len(&self) -> usize {
                match self {
                    $(
                        IoctlRawCmd::$ioctl_name(_) => get_arg_len!($($ioctl_type_tt)*),
                    )*
                    IoctlRawCmd::NonBuiltin(inner) => inner.arg_len(),
                }
            }

            pub fn cmd_num(&self) -> u32 {
                match self {
                    $(
                        IoctlRawCmd::$ioctl_name(_) => BuiltinIoctlNum::$ioctl_name as u32,
                    )*
                    IoctlRawCmd::NonBuiltin(inner) => inner.cmd_num().as_u32(),
                }
            }
        }
    }
}

macro_rules! has_arg {
    (()) => {
        false
    };
    ($($ioctl_type_tt: tt)*) => {
        true
    };
}

macro_rules! get_arg_type {
    (()) => {
        ()
    };
    ($($ioctl_type_tt: tt)*) => {
        &'a $($ioctl_type_tt)*
    };
}

macro_rules! get_arg_len {
    (()) => {
        0
    };
    (mut $type: ty) => {
        std::mem::size_of::<$type>()
    };
    ($type: ty) => {
        std::mem::size_of::<$type>()
    };
}

macro_rules! get_arg {
    ((), $arg_ptr: ident) => {
        ()
    };
    (mut $type: ty, $arg_ptr: ident) => {
        unsafe { &mut *($arg_ptr as *mut $type) }
    };
    ($type: ty, $arg_ptr: ident) => {
        unsafe { &*($arg_ptr as *const $type) }
    };
}

macro_rules! get_arg_ptr {
    ((), $arg_ref: ident) => {
        std::ptr::null() as *const u8
    };
    (mut $type: ty, $arg_ref: ident) => {
        (*$arg_ref as *const $type) as *const u8
    };
    ($type: ty, $arg_ref: ident) => {
        (*$arg_ref as *const $type) as *const u8
    };
}
