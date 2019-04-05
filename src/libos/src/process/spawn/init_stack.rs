use super::*;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use {std, std::mem, std::ptr};

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
    auxtbl: &AuxTable,
) -> Result<usize, Error> {
    let stack_buf = unsafe { StackBuf::new(stack_top, init_area_size)? };
    let envp_cloned = clone_cstrings_on_stack(&stack_buf, envp)?;
    let argv_cloned = clone_cstrings_on_stack(&stack_buf, argv)?;
    dump_auxtbl_on_stack(&stack_buf, auxtbl)?;
    dump_cstrptrs_on_stack(&stack_buf, &envp_cloned);
    dump_cstrptrs_on_stack(&stack_buf, &argv_cloned);
    stack_buf.put(argv.len() as u64);
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
    pub unsafe fn new(stack_top: usize, stack_size: usize) -> Result<StackBuf, Error> {
        if stack_top % 16 != 0 || stack_size == 0 || stack_top < stack_size {
            return errno!(EINVAL, "Invalid stack range");
        };
        Ok(StackBuf {
            stack_top: stack_top,
            stack_bottom: stack_top - stack_size,
            stack_pos: Cell::new(stack_top),
        })
    }

    pub fn put<T>(&self, val: T) -> Result<*const T, Error>
    where
        T: Copy,
    {
        let val_size = mem::size_of::<T>();
        let val_align = mem::align_of::<T>();
        let val_ptr = self.alloc(val_size, val_align)? as *mut T;
        unsafe {
            ptr::write(val_ptr, val);
        }
        Ok(val_ptr as *const T)
    }

    pub fn put_slice<T>(&self, vals: &[T]) -> Result<*const T, Error>
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

    pub fn put_cstr(&self, cstr: &CStr) -> Result<*const u8, Error> {
        let bytes = cstr.to_bytes_with_nul();
        self.put_slice(bytes)
    }

    pub fn get_pos(&self) -> usize {
        self.stack_pos.get()
    }

    fn alloc(&self, size: usize, align: usize) -> Result<*mut u8, Error> {
        let new_pos = {
            let old_pos = self.stack_pos.get();
            let new_pos = align_down(old_pos - size, align);
            if new_pos < self.stack_bottom {
                return Err(Error::new(Errno::ENOMEM, "No enough space in buffer"));
            }
            new_pos
        };
        self.stack_pos.set(new_pos);

        Ok(new_pos as *mut u8)
    }
}

fn clone_cstrings_on_stack<'a, 'b>(
    stack: &'a StackBuf,
    cstrings: &'b [CString],
) -> Result<Vec<&'a CStr>, Error> {
    let mut cstrs_cloned = Vec::new();
    for cs in cstrings.iter().rev() {
        let cstrp_cloned = stack.put_cstr(cs)?;
        let cstr_cloned = unsafe { CStr::from_ptr::<'a>(cstrp_cloned as *const c_char) };
        cstrs_cloned.push(cstr_cloned);
    }
    cstrs_cloned.reverse();
    Ok(cstrs_cloned)
}

fn dump_auxtbl_on_stack<'a, 'b>(stack: &'a StackBuf, auxtbl: &'b AuxTable) -> Result<(), Error> {
    // For every key-value pari, dump the value first, then the key
    stack.put(AuxKey::AT_NULL as u64);
    stack.put(AuxKey::AT_NULL as u64);
    for (aux_key, aux_val) in auxtbl {
        stack.put(aux_val);
        stack.put(aux_key as u64);
    }
    Ok(())
}

fn dump_cstrptrs_on_stack<'a, 'b>(
    stack: &'a StackBuf,
    strptrs: &'b [&'a CStr],
) -> Result<(), Error> {
    stack.put(0 as u64); // End with a NULL pointer
    for sp in strptrs.iter().rev() {
        stack.put(sp.as_ptr() as u64);
    }
    Ok(())
}

/* Symbolic values for the entries in the auxiliary table
put on the initial stack */
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AuxKey {
    AT_NULL = 0,      /* end of vector */
    AT_IGNORE = 1,    /* entry should be ignored */
    AT_EXECFD = 2,    /* file descriptor of program */
    AT_PHDR = 3,      /* program headers for program */
    AT_PHENT = 4,     /* size of program header entry */
    AT_PHNUM = 5,     /* number of program headers */
    AT_PAGESZ = 6,    /* system page size */
    AT_BASE = 7,      /* base address of interpreter */
    AT_FLAGS = 8,     /* flags */
    AT_ENTRY = 9,     /* entry point of program */
    AT_NOTELF = 10,   /* program is not ELF */
    AT_UID = 11,      /* real uid */
    AT_EUID = 12,     /* effective uid */
    AT_GID = 13,      /* real gid */
    AT_EGID = 14,     /* effective gid */
    AT_PLATFORM = 15, /* string identifying CPU for optimizations */
    AT_HWCAP = 16,    /* arch dependent hints at CPU capabilities */
    AT_CLKTCK = 17,   /* frequency at which times() increments */

    /* 18...22 not used */
    AT_SECURE = 23, /* secure mode boolean */
    AT_BASE_PLATFORM = 24, /* string identifying real platform, may
                     * differ from AT_PLATFORM. */
    AT_RANDOM = 25, /* address of 16 random bytes */
    AT_HWCAP2 = 26, /* extension of AT_HWCAP */

    /* 28...30 not used */
    AT_EXECFN = 31, /* filename of program */
}

static AUX_KEYS: &'static [AuxKey] = &[
    AuxKey::AT_NULL,
    AuxKey::AT_IGNORE,
    AuxKey::AT_EXECFD,
    AuxKey::AT_PHDR,
    AuxKey::AT_PHENT,
    AuxKey::AT_PHNUM,
    AuxKey::AT_PAGESZ,
    AuxKey::AT_BASE,
    AuxKey::AT_FLAGS,
    AuxKey::AT_ENTRY,
    AuxKey::AT_NOTELF,
    AuxKey::AT_UID,
    AuxKey::AT_EUID,
    AuxKey::AT_GID,
    AuxKey::AT_EGID,
    AuxKey::AT_PLATFORM,
    AuxKey::AT_HWCAP,
    AuxKey::AT_CLKTCK,
    AuxKey::AT_SECURE,
    AuxKey::AT_BASE_PLATFORM,
    AuxKey::AT_RANDOM,
    AuxKey::AT_HWCAP2,
    AuxKey::AT_EXECFN,
];

impl AuxKey {
    pub const MAX: usize = 32;

    pub fn next(&self) -> Option<AuxKey> {
        let self_idx = AUX_KEYS.iter().position(|x| *x == *self).unwrap();
        let next_idx = self_idx + 1;
        if next_idx < AUX_KEYS.len() {
            Some(AUX_KEYS[next_idx])
        } else {
            None
        }
    }
}

#[derive(Clone, Default, Copy, Debug)]
pub struct AuxTable {
    values: [Option<u64>; AuxKey::MAX],
}

impl AuxTable {
    pub fn new() -> AuxTable {
        AuxTable {
            values: [None; AuxKey::MAX],
        }
    }

    pub fn set_val(&mut self, key: AuxKey, val: u64) -> Result<(), Error> {
        if key == AuxKey::AT_NULL || key == AuxKey::AT_IGNORE {
            return Err(Error::new(Errno::EINVAL, "Illegal key"));
        }
        self.values[key as usize] = Some(val);
        Ok(())
    }

    pub fn get_val(&self, key: AuxKey) -> Option<u64> {
        self.values[key as usize]
    }

    pub fn del_val(&mut self, key: AuxKey) {
        self.values[key as usize] = None;
    }

    pub fn iter<'a>(&'a self) -> AuxTableIter<'a> {
        AuxTableIter {
            tbl: self,
            key: Some(AuxKey::AT_NULL),
        }
    }
}

impl<'a> IntoIterator for &'a AuxTable {
    type Item = (AuxKey, u64);
    type IntoIter = AuxTableIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

pub struct AuxTableIter<'a> {
    tbl: &'a AuxTable,
    key: Option<AuxKey>,
}

impl<'a> Iterator for AuxTableIter<'a> {
    type Item = (AuxKey, u64);

    fn next(&mut self) -> Option<(AuxKey, u64)> {
        loop {
            if self.key == None {
                return None;
            }
            let key = self.key.unwrap();

            let item = self.tbl.get_val(key).map(|val| (key, val));
            self.key = key.next();

            if item != None {
                return item;
            }
        }
    }
}
