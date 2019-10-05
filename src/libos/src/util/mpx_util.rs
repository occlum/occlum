use super::*;

pub fn mpx_enable() -> Result<()> {
    match unsafe { __mpx_enable() } {
        0 => Ok(()),
        _ => Err(errno!(EPERM, "MPX cannot be enabled")),
    }
}

pub enum MpxReg {
    BND0,
    BND1,
    BND2,
    BND3,
}

pub fn mpx_bndmk(bndreg: MpxReg, base: usize, size: usize) -> Result<()> {
    /* Check whether the upper bound overflows the max of 64-bit */
    if base.checked_add(size).is_none() {
        return_errno!(ERANGE, "Upper bound overflows");
    }

    match bndreg {
        MpxReg::BND0 => unsafe { __mpx_bndmk0(base, size) },
        MpxReg::BND1 => unsafe { __mpx_bndmk1(base, size) },
        MpxReg::BND2 => unsafe { __mpx_bndmk2(base, size) },
        MpxReg::BND3 => unsafe { __mpx_bndmk3(base, size) },
    }
    Ok(())
}

pub fn mpx_bndcl(bndreg: MpxReg, addr: usize) {
    match bndreg {
        MpxReg::BND0 => unsafe { __mpx_bndcl0(addr) },
        MpxReg::BND1 => unsafe { __mpx_bndcl1(addr) },
        MpxReg::BND2 => unsafe { __mpx_bndcl2(addr) },
        MpxReg::BND3 => unsafe { __mpx_bndcl3(addr) },
    }
}

pub fn mpx_bndcu(bndreg: MpxReg, addr: usize) {
    match bndreg {
        MpxReg::BND0 => unsafe { __mpx_bndcu0(addr) },
        MpxReg::BND1 => unsafe { __mpx_bndcu1(addr) },
        MpxReg::BND2 => unsafe { __mpx_bndcu2(addr) },
        MpxReg::BND3 => unsafe { __mpx_bndcu3(addr) },
    }
}

extern "C" {
    // See mpx_util.h
    fn __mpx_enable() -> i32;
    fn __mpx_bndmk0(base: usize, size: usize);
    fn __mpx_bndmk1(base: usize, size: usize);
    fn __mpx_bndmk2(base: usize, size: usize);
    fn __mpx_bndmk3(base: usize, size: usize);
    fn __mpx_bndcl0(x: usize);
    fn __mpx_bndcl1(x: usize);
    fn __mpx_bndcl2(x: usize);
    fn __mpx_bndcl3(x: usize);
    fn __mpx_bndcu0(x: usize);
    fn __mpx_bndcu1(x: usize);
    fn __mpx_bndcu2(x: usize);
    fn __mpx_bndcu3(x: usize);
}
