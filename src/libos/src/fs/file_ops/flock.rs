use super::*;

pub fn do_flock(fd: FileDesc, ops: FlockOps) -> Result<()> {
    debug!("flock: fd: {}, ops: {:?}", fd, ops);

    let file_ref = current!().file(fd)?;
    let inode_file = file_ref.as_inode_file()?;
    if ops.contains(FlockOps::LOCK_UN) {
        inode_file.unlock_flock();
    } else {
        let is_nonblocking = ops.contains(FlockOps::LOCK_NB);
        let flock = {
            let type_ = FlockType::from(ops);
            Flock::new(&file_ref, type_)
        };
        inode_file.set_flock(flock, is_nonblocking)?;
    }
    Ok(())
}
