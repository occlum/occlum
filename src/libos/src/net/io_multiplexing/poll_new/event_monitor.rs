use std::cell::Cell;
use std::ptr;
use std::sync::Weak;
use std::time::Duration;

use crate::events::{Observer, Waiter, WaiterQueueObserver};
use crate::fs::{AtomicIoEvents, IoEvents};
use crate::prelude::*;
use crate::time::{timespec_t, TIMERSLACK};

/// Monitor events that happen on a set of interesting files.
///
/// The event monitor can wait for events on both LibOS files and host files.
/// Event better, as a result of waiting for the events of host files, the
/// states of host files (returned by the `poll` method) are also updated.
pub struct EventMonitor {
    // The set of interesting files and their events.
    files_and_events: Vec<(FileRef, IoEvents)>,
    // The indexes of host files inside the set of the interesting files.
    host_file_idxes: Vec<usize>,
    // An array of struct pollfd as the argument for the poll syscall via OCall.
    //
    // The items in `ocall_pollfds` corresponds to the items in
    // `host_file_idxes` on a one-on-one basis, except the last item in
    // `ocall_pollfds`. This indicates that `ocall_pollfds.len() ==
    // host_files_idxes.len() + 1`.
    ocall_pollfds: Vec<libc::pollfd>,
    // An observer and also a waiter queue.
    observer: Arc<WaiterQueueObserver<IoEvents>>,
    // A waiter.
    //
    // The last two fields comprise of a common pattern enabled by the event
    // subsystem.
    waiter: Waiter,
}

impl EventMonitor {
    /// Returns an iterator for the set of the interesting files.
    pub fn files(&self) -> impl Iterator<Item = &FileRef> {
        self.files_and_events.iter().map(|(file, _)| file)
    }

    /// Returns an iterator for the host files in the set of the interesting files.
    pub fn host_files_and_events(&self) -> impl Iterator<Item = &(FileRef, IoEvents)> {
        self.host_file_idxes
            .iter()
            .map(move |idx| &self.files_and_events[*idx])
    }

    /// Reset the monitor so that it can wait for new events.
    pub fn reset_events(&mut self) {
        self.observer.waiter_queue().reset_and_enqueue(&self.waiter);
    }

    /// Wait for some interesting events that happen on the set of files.
    ///
    /// To make the code more efficient, this method also polls the states of
    /// the host files in the set and updates their states accordingly.
    ///
    /// The signature of this method gets a bit of complicated to fight with
    /// Rust's move semantics. The ownership of the `timeout` argument is moved
    /// from the caller to this function. To give the `timeout` argument back to
    /// the caller (so that he or she can repeatedly use the argument in a
    /// loop), we return the `timeout` inside `Result::Ok`.
    pub fn wait_events<'a, 'b>(
        &'a mut self,
        mut timeout: Option<&'b mut Duration>,
    ) -> Result<Option<&'b mut Duration>> {
        const ZERO: Duration = Duration::from_secs(0);
        if let Some(timeout) = timeout.as_ref() {
            if **timeout == ZERO {
                return_errno!(ETIMEDOUT, "should return immediately");
            }
        }

        // The do_ocall method returns when one of the following conditions is satisfied:
        // 1. self.waiter is waken, indicating some interesting events happen on the LibOS files;
        // 2. some interesting events happen on the host files;
        // 3. a signal arrives;
        // 4. the time is up.
        let num_events = self.do_poll_ocall(&mut timeout)?;

        self.update_host_file_events();

        // Poll syscall does not treat timeout as error. So we need
        // to distinguish the case by ourselves.
        if let Some(timeout) = timeout.as_mut() {
            if num_events == 0 {
                **timeout = ZERO;
                return_errno!(ETIMEDOUT, "no results and the time is up");
            }
        }
        assert!(num_events > 0);

        Ok(timeout)
    }

    /// Poll the host files among the set of the interesting files and update
    /// their states accordingly.
    pub fn poll_host_files(&mut self) {
        let mut zero_timeout = Some(Duration::from_secs(0));
        if let Err(_) = self.do_poll_ocall(&mut zero_timeout.as_mut()) {
            return;
        }

        self.update_host_file_events();
    }

    fn do_poll_ocall(&mut self, timeout: &mut Option<&mut Duration>) -> Result<usize> {
        extern "C" {
            fn occlum_ocall_poll_with_eventfd(
                ret: *mut i32,
                fds: *mut libc::pollfd,
                nfds: u32,
                timeout: *mut timespec_t,
                eventfd_idx: i32,
            ) -> sgx_status_t;
        }

        // Do poll syscall via OCall
        let num_events = try_libc!({
            let mut remain_c = timeout.as_ref().map(|timeout| timespec_t::from(**timeout));
            let remain_c_ptr = remain_c.as_mut().map_or(ptr::null_mut(), |mut_ref| mut_ref);

            let host_eventfd_idx = self.ocall_pollfds.len() - 1;

            let mut ret = 0;
            let status = unsafe {
                occlum_ocall_poll_with_eventfd(
                    &mut ret,
                    (&mut self.ocall_pollfds[..]).as_mut_ptr(),
                    self.ocall_pollfds.len() as u32,
                    remain_c_ptr,
                    host_eventfd_idx as i32,
                )
            };
            assert!(status == sgx_status_t::SGX_SUCCESS);

            if let Some(timeout) = timeout.as_mut() {
                let remain = remain_c.unwrap().as_duration();
                assert!(remain <= **timeout + TIMERSLACK.to_duration());
                **timeout = remain;
            }

            ret
        }) as usize;
        Ok(num_events)
    }

    fn update_host_file_events(&self) {
        // According to the output pollfds, update the states of the corresponding host files
        let output_pollfds = self.ocall_pollfds[..self.ocall_pollfds.len() - 1].iter();
        for (pollfd, (host_file, mask)) in output_pollfds.zip(self.host_files_and_events()) {
            let revents = {
                assert!((pollfd.revents & libc::POLLNVAL) == 0);
                IoEvents::from_raw(pollfd.revents as u32)
            };
            host_file.update_host_events(&revents, mask, false);
        }
    }
}

impl Drop for EventMonitor {
    fn drop(&mut self) {
        let weak_observer = Arc::downgrade(&self.observer) as Weak<dyn Observer<_>>;
        weak_observer.unregister_files(self.files_and_events.iter());
    }
}

pub struct EventMonitorBuilder {
    files_and_events: Vec<(FileRef, IoEvents)>,
    host_file_idxes: Vec<usize>,
    ocall_pollfds: Vec<libc::pollfd>,
    observer: Arc<WaiterQueueObserver<IoEvents>>,
    waiter: Waiter,
}

impl EventMonitorBuilder {
    pub fn new(expected_num_files: usize) -> Self {
        let files_and_events = Vec::with_capacity(expected_num_files);
        let host_file_idxes = Vec::new();
        let ocall_pollfds = Vec::new();
        let observer = WaiterQueueObserver::new();
        let waiter = Waiter::new();
        Self {
            files_and_events,
            host_file_idxes,
            ocall_pollfds,
            observer,
            waiter,
        }
    }

    pub fn add_file(&mut self, file: FileRef, events: IoEvents) {
        if file.host_fd().is_some() {
            let host_file_idx = self.files_and_events.len();
            self.host_file_idxes.push(host_file_idx);
        }

        self.files_and_events.push((file, events));
    }

    fn init_ocall_pollfds(&mut self) {
        let ocall_pollfds = &mut self.ocall_pollfds;

        // For each host file, add a corresponding pollfd item
        let files_and_events = &self.files_and_events;
        let host_files_and_events = self
            .host_file_idxes
            .iter()
            .map(move |idx| &files_and_events[*idx]);
        for (file, events) in host_files_and_events {
            ocall_pollfds.push(libc::pollfd {
                fd: file.host_fd().unwrap().to_raw() as i32,
                events: events.to_raw() as i16,
                revents: 0,
            });
        }

        // Add one more for waiter's underlying host eventfd
        ocall_pollfds.push(libc::pollfd {
            fd: self.waiter.host_eventfd().host_fd() as i32,
            events: libc::POLLIN,
            revents: 0,
        });
    }

    fn init_observer(&self) {
        let weak_observer = Arc::downgrade(&self.observer) as Weak<dyn Observer<_>>;
        weak_observer.register_files(self.files_and_events.iter());
    }

    pub fn build(mut self) -> EventMonitor {
        self.init_ocall_pollfds();
        self.init_observer();

        let mut new_event_monitor = {
            let Self {
                files_and_events,
                host_file_idxes,
                ocall_pollfds,
                observer,
                waiter,
            } = self;
            EventMonitor {
                files_and_events,
                host_file_idxes,
                ocall_pollfds,
                observer,
                waiter,
            }
        };
        new_event_monitor.poll_host_files();
        new_event_monitor
    }
}

// An extention trait to make registering/unregistering an observer for a
// bunch of files easy.
trait ObserverExt {
    fn register_files<'a>(&self, files_and_events: impl Iterator<Item = &'a (FileRef, IoEvents)>);
    fn unregister_files<'a>(&self, files_and_events: impl Iterator<Item = &'a (FileRef, IoEvents)>);
}

impl ObserverExt for Weak<dyn Observer<IoEvents>> {
    fn register_files<'a>(&self, files_and_events: impl Iterator<Item = &'a (FileRef, IoEvents)>) {
        for (file, events) in files_and_events {
            let notifier = match file.notifier() {
                None => continue,
                Some(notifier) => notifier,
            };

            let mask = *events;
            notifier.register(self.clone(), Some(mask), None);
        }
    }

    fn unregister_files<'a>(
        &self,
        files_and_events: impl Iterator<Item = &'a (FileRef, IoEvents)>,
    ) {
        for (file, events) in files_and_events {
            let notifier = match file.notifier() {
                None => continue,
                Some(notifier) => notifier,
            };
            notifier.unregister(self);
        }
    }
}
