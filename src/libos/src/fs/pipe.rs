use super::*;

pub use async_io::util::channel::{Channel, Consumer as PipeReader, Producer as PipeWriter};

// TODO: Add F_SETPIPE_SZ in fcntl to dynamically change the size of pipe
// to improve memory efficiency. This value is got from /proc/sys/fs/pipe-max-size on linux.
pub const DEFAULT_BUF_SIZE: usize = 1024 * 1024;

pub fn do_pipe2(flags: u32) -> Result<[FileDesc; 2]> {
    let creation_flags = CreationFlags::from_bits_truncate(flags);
    let status_flags = StatusFlags::from_bits_truncate(flags);
    debug!("pipe2: flags: {:?} {:?}", creation_flags, status_flags);

    let (pipe_writer, pipe_reader) = pipe(status_flags)?;
    let pipe_writer = FileRef::new_file(pipe_writer);
    let pipe_reader = FileRef::new_file(pipe_reader);

    let close_on_spawn = creation_flags.must_close_on_spawn();
    let current = current!();
    let reader_fd = current.add_file(pipe_reader, close_on_spawn);
    let writer_fd = current.add_file(pipe_writer, close_on_spawn);
    debug!(
        "pipe2: returns reader_fd = {}, writer_fd = {}",
        reader_fd, writer_fd
    );
    Ok([reader_fd, writer_fd])
}

fn pipe(flags: StatusFlags) -> Result<(PipeWriter, PipeReader)> {
    let channel = Channel::with_capacity_and_flags(DEFAULT_BUF_SIZE, flags)?;
    let (producer, consumer) = channel.split();
    Ok((producer, consumer))
}
