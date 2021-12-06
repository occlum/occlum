//! Block I/O (BIO).

use core::fmt::{self};
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::pin::Pin;
use core::task::Waker;
use object_id::ObjectId;

use crate::prelude::*;

/// The type of a block request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BioType {
    /// A read request.
    Read,
    /// A write request.
    Write,
    /// A flush request.
    Flush,
}

/// A request to a block device.
pub struct BioReq {
    id: ObjectId,
    type_: BioType,
    addr: BlockId,
    bufs: Mutex<Vec<BlockBuf>>,
    inner: Mutex<Inner>,
}

/// A response from a block device.
pub type BioResp = core::result::Result<(), Errno>;

/// The closure type for the callback function invoked upon the completion of
/// a request to a block device.
pub type BioCompletionCallback = Box<dyn FnOnce(/* req = */ &BioReq, /* resp = */ BioResp)>;

struct Inner {
    waker: Option<Waker>,
    status: Status,
    callback: Option<BioCompletionCallback>,
}

enum Status {
    Init,
    Submitted,
    Completed(BioResp),
}

impl Inner {
    fn new(callback: Option<BioCompletionCallback>) -> Self {
        Self {
            waker: None,
            status: Status::Init,
            callback,
        }
    }
}

impl BioReq {
    /// Create a read request for a block device.
    pub fn new_read(
        addr: BlockId,
        bufs: Vec<BlockBuf>,
        callback: Option<BioCompletionCallback>,
    ) -> Result<Self> {
        let type_ = BioType::Read;
        Self::do_new(type_, addr, bufs, callback)
    }

    /// Create a write request for a block device.
    pub fn new_write(
        addr: BlockId,
        bufs: Vec<BlockBuf>,
        callback: Option<BioCompletionCallback>,
    ) -> Result<Self> {
        let type_ = BioType::Write;
        Self::do_new(type_, addr, bufs, callback)
    }

    /// Create a flush request for a block device.
    pub fn new_flush(callback: Option<BioCompletionCallback>) -> Result<Self> {
        let type_ = BioType::Flush;
        let addr = BlockId::max_value();
        let bufs = Vec::new();
        Self::do_new(type_, addr, bufs, callback)
    }

    fn do_new(
        type_: BioType,
        addr: BlockId,
        bufs: Vec<BlockBuf>,
        callback: Option<BioCompletionCallback>,
    ) -> Result<Self> {
        if addr.checked_add(bufs.len() as u64).is_none() {
            return Err(errno!(EINVAL, "addr overflow"));
        }

        let inner = Inner::new(callback);
        let new_self = Self {
            id: ObjectId::new(),
            type_,
            addr,
            bufs: Mutex::new(bufs),
            inner: Mutex::new(inner),
        };
        Ok(new_self)
    }

    /// Returns a unique ID of the request.
    pub fn id(&self) -> u64 {
        self.id.get()
    }

    /// Returns the type of the request.
    pub fn type_(&self) -> BioType {
        self.type_
    }

    /// Returns the starting address of requested blocks.
    ///
    /// The return value is meaningless if the request is not a read or write.
    pub fn addr(&self) -> BlockId {
        self.addr
    }

    /// Access the immutable buffers with a closure.
    pub fn bufs_with<F>(&self, mut f: F)
    where
        F: FnMut(&[BlockBuf]),
    {
        let bufs = self.bufs.lock();
        (f)(&bufs)
    }

    /// Access the mutable buffers with a closure.
    pub fn mut_bufs_with<F>(&self, mut f: F)
    where
        F: FnMut(&mut [BlockBuf]),
    {
        let mut bufs = self.bufs.lock();
        (f)(&mut bufs)
    }

    /// Returns the number of block buffers.
    pub fn num_bufs(&self) -> usize {
        self.bufs.lock().len()
    }

    /// Returns the response to the request.
    ///
    /// If the request is completed, the method returns `Some(resp)`, where `resp`
    /// is the response to the request. Otherwise, the method returns `None`.
    pub fn response(&self) -> Option<BioResp> {
        let inner = self.inner.lock();
        match inner.status {
            Status::Completed(res) => Some(res),
            _ => None,
        }
    }

    /// Update the status of the request to "completed" by giving the response
    /// to the request.
    ///
    /// # Safety
    ///
    /// After the invoking this API, the request is considered completed, which
    /// means the request must have taken effect. For example, a completed read
    /// request must have all its buffers filled with data. This API should only
    /// be invoked by an implementation of block device. We mark this API unsafe
    /// to prevent user code from calling it mistakenly.
    pub unsafe fn complete(&self, resp: BioResp) {
        let mut inner = self.inner.lock();
        match &inner.status {
            Status::Submitted => {
                let waker = inner.waker.take();
                inner.status = Status::Completed(resp);
                let callback = inner.callback.take();
                drop(inner);

                if let Some(callback) = callback {
                    (callback)(self, resp);
                }
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
            _ => panic!("cannot complete before submitting or complete twice"),
        }
    }

    /// Update the status of the request to "submitted".
    fn submit(&self) {
        let mut inner = self.inner.lock();
        match &inner.status {
            Status::Init => {
                inner.status = Status::Submitted;
                drop(inner);
            }
            _ => panic!("cannot complete before submitting or complete twice"),
        }
    }
}

impl fmt::Debug for BioReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut ds = f.debug_struct("BioReq");
        ds.field("id", &self.id());
        ds.field("type", &self.type_());
        if self.type_() == BioType::Read || self.type_() == BioType::Write {
            ds.field("addr", &self.addr());
            ds.field("num_bufs", &self.num_bufs());
        }
        ds.field("resp", &self.response());
        ds.finish()
    }
}

impl PartialEq for BioReq {
    fn eq(&self, other: &Self) -> bool {
        self.id() == other.id()
    }
}

impl Eq for BioReq {}

impl Hash for BioReq {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id().hash(state);
    }
}

/// A submitted block request.
#[derive(Debug)]
pub struct BioSubmission {
    req: Arc<BioReq>,
}

impl BioSubmission {
    pub fn new(req: Arc<BioReq>) -> Self {
        req.submit();
        Self { req }
    }

    /// Returns the submitted request.
    pub fn req(&self) -> &Arc<BioReq> {
        &self.req
    }

    /// Returns a future that can be awaited for the completion of the submission.
    pub fn complete(self) -> BioComplete {
        BioComplete::new(self)
    }
}

/// A future for a completed request.
#[must_use = "a future should be used"]
pub struct BioComplete {
    req: Arc<BioReq>,
}

impl BioComplete {
    fn new(submission: BioSubmission) -> Self {
        let BioSubmission { req } = submission;
        Self { req }
    }
}

impl Future for BioComplete {
    type Output = Arc<BioReq>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut inner = self.req.inner.lock();
        match inner.status {
            Status::Submitted => {
                if inner.waker.is_none() {
                    inner.waker = Some(cx.waker().clone());
                }
                Poll::Pending
            }
            Status::Completed(_) => {
                inner.waker = None;
                Poll::Ready(self.req.clone())
            }
            Status::Init => unreachable!("request must be submitted first"),
        }
    }
}
