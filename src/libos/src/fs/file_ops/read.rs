use super::*;

pub fn do_read(fd: FileDesc, buf: &mut [u8]) -> Result<usize> {
    debug!("read: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    file_ref.read(buf)
}

pub fn do_readv(fd: FileDesc, bufs: &mut [&mut [u8]]) -> Result<usize> {
    debug!("readv: fd: {}", fd);
    let file_ref = current!().file(fd)?;
    file_ref.readv(bufs)
}

pub fn do_pread(fd: FileDesc, buf: &mut [u8], offset: off_t) -> Result<usize> {
    debug!("pread: fd: {}, offset: {}", fd, offset);
    if offset < 0 {
        return_errno!(EINVAL, "the offset is negative");
    }
    let file_ref = current!().file(fd)?;
    file_ref.read_at(offset as usize, buf)
}
