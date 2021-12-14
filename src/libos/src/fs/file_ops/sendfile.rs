use super::*;

pub async fn do_sendfile(
    out_fd: FileDesc,
    in_fd: FileDesc,
    offset: Option<off_t>,
    count: usize,
) -> Result<(usize, usize)> {
    // (len, offset)
    debug!(
        "sendfile: out: {}, in: {}, offset: {:?}, count: {}",
        out_fd, in_fd, offset, count
    );

    let current = current!();
    let in_file = current.file(in_fd)?;
    let in_inode_file = in_file
        .as_inode_file()
        .ok_or_else(|| errno!(EINVAL, "not an inode"))?;
    let out_file = current.file(out_fd)?;
    let mut buffer: [u8; 0x1000] = unsafe { MaybeUninit::uninit().assume_init() };

    let mut read_offset = match offset {
        Some(offset) => offset as usize,
        None => in_inode_file.position(),
    };

    // read from specified offset and write new offset back
    let mut bytes_read = 0;
    let mut bytes_write = 0;
    while bytes_read < count {
        let len = buffer.len().min(count - bytes_read);
        let read_len = in_inode_file.read_at(read_offset, &mut buffer[..len])?;
        if read_len == 0 {
            break;
        }
        bytes_read += read_len;
        read_offset += read_len;
        let write_len = out_file.write(&buffer[..read_len]).await?;
        bytes_write += write_len;
        if write_len != read_len {
            break;
        }
    }

    // If offset is not none, does not modify file offset of in_fd;
    // otherwise the file offset is adjusted to reflect the number of bytes read
    if offset.is_none() {
        in_inode_file.seek(SeekFrom::Current(bytes_read as i64))?;
    }
    Ok((bytes_write, read_offset))
}
