use super::*;
use crate::fs::EventFile;

lazy_static! {
    pub static ref THREAD_NOTIFIERS: SgxMutex<HashMap<pid_t, EventFile>> =
        SgxMutex::new(HashMap::new());
}

#[derive(Debug)]
pub enum IoEvent {
    Poll(PollEvent),
    Epoll(EpollEvent),
    BlockingRead,
    BlockingWrite,
}

pub fn notify_thread(tid: pid_t) -> Result<()> {
    debug!("notify thread {}", tid);
    assert_ne!(
        tid,
        current!().tid(),
        "a waiting thread cannot run other programs"
    );
    let data: &[u8] = &[1, 0, 0, 0, 0, 0, 0, 0];

    THREAD_NOTIFIERS
        .lock()
        .unwrap()
        .get(&tid)
        .unwrap()
        .write(&data)?;
    Ok(())
}

pub fn clear_notifier_status(tid: pid_t) -> Result<()> {
    // One can only clear self for now
    assert_eq!(tid, current!().tid());
    debug!("clear thread {} notifier", tid);
    let mut data: &mut [u8] = &mut [0; 8];

    // Ignore the error for no data to read
    THREAD_NOTIFIERS
        .lock()
        .unwrap()
        .get(&tid)
        .unwrap()
        .read(&mut data);
    Ok(())
}

pub fn wait_for_notification() -> Result<()> {
    do_poll(&mut vec![], std::ptr::null_mut())?;
    Ok(())
}
