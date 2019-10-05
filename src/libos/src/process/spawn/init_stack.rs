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
) -> Result<usize> {
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

fn dump_auxtbl_on_stack<'a, 'b>(stack: &'a StackBuf, auxtbl: &'b AuxTable) -> Result<()> {
    // For every key-value pair, dump the value first, then the key
    stack.put(0 as u64);
    stack.put(AuxKey::AT_NULL as u64);
    for (aux_key, aux_val) in auxtbl.table() {
        stack.put(*aux_val as u64);
        stack.put(*aux_key as u64);
    }
    Ok(())
}

fn dump_cstrptrs_on_stack<'a, 'b>(stack: &'a StackBuf, strptrs: &'b [&'a CStr]) -> Result<()> {
    stack.put(0 as u64); // End with a NULL pointer
    for sp in strptrs.iter().rev() {
        stack.put(sp.as_ptr() as u64);
    }
    Ok(())
}

/* Symbolic values for the entries in the auxiliary table
put on the initial stack */
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
    AT_SYSINFO = 32,

    /* Occlum-specific entries */
    AT_OCCLUM_ENTRY = 48, /* the entry point of Occlum, i.e., syscall */
}

#[derive(Clone, Default, Debug)]
pub struct AuxTable {
    table: HashMap<AuxKey, u64>,
}

impl AuxTable {
    pub fn new() -> AuxTable {
        AuxTable {
            table: HashMap::new(),
        }
    }
}

impl AuxTable {
    pub fn set(&mut self, key: AuxKey, val: u64) -> Result<()> {
        if key == AuxKey::AT_NULL || key == AuxKey::AT_IGNORE {
            return_errno!(EINVAL, "Illegal key");
        }
        self.table
            .entry(key)
            .and_modify(|val_mut| *val_mut = val)
            .or_insert(val);
        Ok(())
    }

    pub fn get(&self, key: AuxKey) -> Option<u64> {
        self.table.get(&key).map(|val_ref| *val_ref)
    }

    pub fn del(&mut self, key: AuxKey) -> Option<u64> {
        self.table.remove(&key)
    }

    pub fn table(&self) -> &HashMap<AuxKey, u64> {
        &self.table
    }
}
