use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::{mem, ptr};

use super::aux_vec::{AuxKey, AuxVec};
use crate::misc;
use crate::prelude::*;

/*
 * The initial stack of a process looks like below:
 *
 *
 *  +---------------------+ <------+ Top of stack
 *  |                     |          (high address)
 *  | Null-terminated     |
 *  | strings referenced  |
 *  | by variables below  |
 *  |                     |
 *  +---------------------+
 *  | AT_NULL             |
 *  +---------------------+
 *  | AT_NULL             |
 *  +---------------------+
 *  | ...                 |
 *  +---------------------+
 *  | aux_val[0]          |
 *  +---------------------+
 *  | aux_key[0]          | <------+ Auxiliary table
 *  +---------------------+
 *  | NULL                |
 *  +---------------------+
 *  | ...                 |
 *  +---------------------+
 *  | char* envp[0]       | <------+ Environment variabls
 *  +---------------------+
 *  | NULL                |
 *  +---------------------+
 *  | char* argv[argc-1]  |
 *  +---------------------+
 *  | ...                 |
 *  +---------------------+
 *  | char* argv[0]       |
 *  +---------------------+
 *  | long argc           | <------+ Program arguments
 *  +---------------------+
 *  |                     |
 *  |                     |
 *  +                     +
 *
 */

pub fn do_init(
    stack_top: usize,
    init_area_size: usize,
    argv: &[CString],
    envp: &[CString],
    auxtbl: &mut AuxVec,
) -> Result<usize> {
    let stack_buf = unsafe { StackBuf::new(stack_top, init_area_size)? };
    let envp_cloned = clone_cstrings_on_stack(&stack_buf, envp)?;
    let argv_cloned = clone_cstrings_on_stack(&stack_buf, argv)?;
    let rand_val_ptr = generate_random_on_stack(&stack_buf)?;
    auxtbl.set(AuxKey::AT_RANDOM, rand_val_ptr as *const () as u64);
    adjust_alignment(&stack_buf, auxtbl, &envp_cloned, &argv_cloned)?;
    dump_auxtbl_on_stack(&stack_buf, auxtbl)?;
    dump_cstrptrs_on_stack(&stack_buf, &envp_cloned)?;
    dump_cstrptrs_on_stack(&stack_buf, &argv_cloned)?;
    stack_buf.put(argv.len() as u64)?;
    Ok(stack_buf.get_pos())
}

/// StackBuf is a buffer that is filled in from high addresses to low
/// (just as a stack). The range of available memory of a StackBuf is from
/// [self.bottom, self.top).
#[derive(Debug)]
pub struct StackBuf {
    stack_top: usize,
    stack_bottom: usize,
    stack_pos: Cell<usize>,
}

impl StackBuf {
    pub unsafe fn new(stack_top: usize, stack_size: usize) -> Result<StackBuf> {
        if stack_top % 16 != 0 || stack_size == 0 || stack_top < stack_size {
            return_errno!(EINVAL, "Invalid stack range");
        };
        Ok(StackBuf {
            stack_top: stack_top,
            stack_bottom: stack_top - stack_size,
            stack_pos: Cell::new(stack_top),
        })
    }

    pub fn put(&self, val: u64) -> Result<*const u64> {
        let val_ptr = self.alloc(8, 8)? as *mut u64;
        unsafe {
            ptr::write(val_ptr, val);
        }
        Ok(val_ptr as *const u64)
    }

    pub fn put_slice<T>(&self, vals: &[T]) -> Result<*const T>
    where
        T: Copy,
    {
        let val_size = mem::size_of::<T>();
        let val_align = mem::align_of::<T>();
        let total_size = {
            let num_vals = vals.len();
            if num_vals == 0 {
                return Ok(self.get_pos() as *const T);
            }
            val_size * num_vals
        };
        let base_ptr = self.alloc(total_size, val_align)? as *mut T;

        let mut val_ptr = base_ptr;
        for v in vals {
            unsafe {
                ptr::write(val_ptr, *v);
            }
            val_ptr = unsafe { val_ptr.offset(1) };
        }

        Ok(base_ptr as *const T)
    }

    pub fn put_cstr(&self, cstr: &CStr) -> Result<*const u8> {
        let bytes = cstr.to_bytes_with_nul();
        self.put_slice(bytes)
    }

    pub fn get_pos(&self) -> usize {
        self.stack_pos.get()
    }

    fn alloc(&self, size: usize, align: usize) -> Result<*mut u8> {
        let new_pos = {
            let old_pos = self.stack_pos.get();
            let new_pos = align_down(old_pos - size, align);
            if new_pos < self.stack_bottom {
                return_errno!(ENOMEM, "No enough space in buffer");
            }
            new_pos
        };
        self.stack_pos.set(new_pos);

        Ok(new_pos as *mut u8)
    }
}

fn generate_random_on_stack(stack: &StackBuf) -> Result<*const u8> {
    let rand_val = {
        let mut rand: [u8; 16] = [0; 16];
        misc::get_random(&mut rand)?;
        rand
    };
    stack.put_slice(&rand_val)
}

fn clone_cstrings_on_stack<'a, 'b>(
    stack: &'a StackBuf,
    cstrings: &'b [CString],
) -> Result<Vec<&'a CStr>> {
    let mut cstrs_cloned = Vec::new();
    for cs in cstrings.iter().rev() {
        let cstrp_cloned = stack.put_cstr(cs)?;
        let cstr_cloned = unsafe { CStr::from_ptr::<'a>(cstrp_cloned as *const c_char) };
        cstrs_cloned.push(cstr_cloned);
    }
    cstrs_cloned.reverse();
    Ok(cstrs_cloned)
}

fn adjust_alignment(
    stack: &StackBuf,
    auxtbl: &AuxVec,
    envp: &[&CStr],
    argv: &[&CStr],
) -> Result<()> {
    // Put 8 byte to make the position of stack 8-byte aligned
    stack.put(0 as u64)?;
    let current_pos = stack.get_pos();
    let to_alloc_size = {
        let auxtbl_size = calc_auxtbl_size_on_stack(auxtbl);
        let envp_size = calc_cstrptrs_size_on_stack(&envp);
        let argv_size = calc_cstrptrs_size_on_stack(&argv);
        let argc_size = mem::size_of::<u64>();
        auxtbl_size + envp_size + argv_size + argc_size
    };
    // Libc ABI requires 16-byte alignment of the stack entrypoint.
    // Current position of the stack is 8-byte aligned already, insert 8 byte
    // to meet the requirement if necessary.
    if (current_pos - to_alloc_size) % 16 != 0 {
        stack.put(0 as u64)?;
    }
    Ok(())
}

fn dump_auxtbl_on_stack<'a, 'b>(stack: &'a StackBuf, auxtbl: &'b AuxVec) -> Result<()> {
    // For every key-value pair, dump the value first, then the key
    stack.put(0 as u64)?;
    stack.put(AuxKey::AT_NULL as u64)?;
    for (aux_key, aux_val) in auxtbl.table() {
        stack.put(*aux_val as u64)?;
        stack.put(*aux_key as u64)?;
    }
    Ok(())
}

fn calc_auxtbl_size_on_stack(auxtbl: &AuxVec) -> usize {
    let auxtbl_item_size = mem::size_of::<u64>() * 2;
    (auxtbl.table().len() + 1) * auxtbl_item_size
}

fn dump_cstrptrs_on_stack<'a, 'b>(stack: &'a StackBuf, strptrs: &'b [&'a CStr]) -> Result<()> {
    stack.put(0 as u64)?; // End with a NULL pointer
    for sp in strptrs.iter().rev() {
        stack.put(sp.as_ptr() as u64)?;
    }
    Ok(())
}

fn calc_cstrptrs_size_on_stack(strptrs: &[&CStr]) -> usize {
    let cstrptrs_item_size = mem::size_of::<u64>();
    (strptrs.len() + 1) * cstrptrs_item_size
}
