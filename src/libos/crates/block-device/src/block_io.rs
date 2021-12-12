//! Block I/O (BIO).

use core::fmt::{self};
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::pin::Pin;
use core::task::Waker;
use object_id::ObjectId;

use crate::prelude::*;
use crate::util::anymap::{Any, AnyMap};

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

/// A builder for `BioReq`.
pub struct BioReqBuilder {
    type_: BioType,
    addr: Option<BlockId>,
    bufs: Option<Vec<BlockBuf>>,
    ext: Option<AnyMap>,
    on_complete: Option<BioReqOnCompleteFn>,
    on_drop: Option<BioReqOnDropFn>,
}

impl BioReqBuilder {
    /// Create a builder of a block request of the give type.
    pub fn new(type_: BioType) -> Self {
        Self {
            type_,
            addr: None,
            bufs: None,
            ext: None,
            on_complete: None,
            on_drop: None,
        }
    }

    /// Specify the block address of the request.
    pub fn addr(mut self, addr: BlockId) -> Self {
        self.addr = Some(addr);
        self
    }

    /// Give the buffers of the request.
    pub fn bufs(mut self, bufs: Vec<BlockBuf>) -> Self {
        self.bufs = Some(bufs);
        self
    }

    /// Add an extension object to the request.
    pub fn ext<T: Any + Sized>(mut self, obj: T) -> Self {
        if self.ext.is_none() {
            self.ext = Some(AnyMap::new());
        }
        let _ = self.ext.as_mut().unwrap().insert(obj);
        self
    }

    /// Specify a callback invoked when the request is complete.
    pub fn on_complete(mut self, on_complete: BioReqOnCompleteFn) -> Self {
        self.on_complete = Some(on_complete);
        self
    }

    /// Specify a callback invoked when the request is dropped.
    pub fn on_drop(mut self, on_drop: BioReqOnDropFn) -> Self {
        self.on_drop = Some(on_drop);
        self
    }

    /// Build the request.
    pub fn build(mut self) -> BioReq {
        let type_ = self.type_;
        if ![BioType::Read, BioType::Write].contains(&type_) {
            debug_assert!(
                self.addr.is_some(),
                "addr is only meaningful for a read or write",
            );
            debug_assert!(
                self.bufs.is_some(),
                "bufs is only meaningful for a read or write",
            );
        }

        let addr = self.addr.unwrap_or(0);
        debug_assert!(
            BLOCK_SIZE.saturating_mul(addr) <= isize::MAX as usize,
            "addr is too big"
        );

        let bufs = self.bufs.take().unwrap_or_else(|| Vec::new());
        let num_bytes = bufs
            .iter()
            .map(|buf| buf.len())
            .fold(0_usize, |sum, len| sum.saturating_add(len));
        debug_assert!(num_bytes <= isize::MAX as usize, "# of bytes is too large");
        let num_blocks = num_bytes / BLOCK_SIZE;
        debug_assert!(num_blocks <= u32::MAX as usize, "# of blocks is too large");

        let ext = self.ext.take().unwrap_or_else(|| AnyMap::new());
        let on_complete = self.on_complete.take();
        let on_drop = self.on_drop.take();

        let id = ObjectId::new();
        let inner = Inner::new();

        BioReq {
            id,
            type_,
            addr,
            num_blocks: num_blocks as u32,
            bufs: Mutex::new(bufs),
            inner: Mutex::new(inner),
            ext: Mutex::new(ext),
            on_complete,
            on_drop,
        }
    }
}

/// A block I/O request.
pub struct BioReq {
    id: ObjectId,
    type_: BioType,
    addr: BlockId,
    num_blocks: u32,
    bufs: Mutex<Vec<BlockBuf>>,
    inner: Mutex<Inner>,
    ext: Mutex<AnyMap>,
    on_complete: Option<BioReqOnCompleteFn>,
    on_drop: Option<BioReqOnDropFn>,
}

/// A response from a block device.
pub type BioResp = core::result::Result<(), Errno>;

/// The type of the callback function invoked upon the completion of
/// a block I/O request.
pub type BioReqOnCompleteFn = fn(/* req = */ &BioReq, /* resp = */ &BioResp);

/// The type of the callback function invoked upon the drop of a block I/O
/// request.
pub type BioReqOnDropFn = fn(/* req = */ &BioReq, /* bufs = */ Vec<BlockBuf>);

struct Inner {
    waker: Option<Waker>,
    status: Status,
}

enum Status {
    Init,
    Submitted,
    Completed(BioResp),
}

impl Inner {
    fn new() -> Self {
        Self {
            waker: None,
            status: Status::Init,
        }
    }
}

impl BioReq {
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
    pub fn access_bufs_with<F, R>(&self, mut f: F) -> R
    where
        F: FnMut(&[BlockBuf]) -> R,
    {
        let bufs = self.bufs.lock();
        (f)(&bufs)
    }

    /// Access the mutable buffers with a closure.
    pub fn access_mut_bufs_with<F, R>(&self, mut f: F) -> R
    where
        F: FnMut(&mut [BlockBuf]) -> R,
    {
        let mut bufs = self.bufs.lock();
        (f)(&mut bufs)
    }

    /// Take the buffers out of the request.
    fn take_bufs(&self) -> Vec<BlockBuf> {
        let mut bufs = self.bufs.lock();
        let mut ret_bufs = Vec::new();
        core::mem::swap(&mut *bufs, &mut ret_bufs);
        ret_bufs
    }

    /// Returns the number of buffers associated with the request.
    ///
    /// If the request is a flush, then the returned value is meaningless.
    pub fn num_bufs(&self) -> usize {
        self.bufs.lock().len()
    }

    /// Returns the number of blocks to read or write by this request.
    ///
    /// If the request is a flush, then the returned value is meaningless.
    pub fn num_blocks(&self) -> usize {
        self.num_blocks as usize
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

    /// Returns the extensions of the request.
    ///
    /// The extensions of a request is a set of objects that may be added, removed,
    /// or accessed by block devices and their users. Implemented with `AnyMap`,
    /// each of the extension objects must have a different type. To avoid
    /// conflicts, it is recommened to use only private types for the extension
    /// objects.
    pub fn ext(&self) -> MutexGuard<AnyMap> {
        self.ext.lock()
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
                drop(inner);

                if let Some(on_complete) = self.on_complete {
                    (on_complete)(self, &resp);
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

impl Drop for BioReq {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop {
            let bufs = self.take_bufs();
            (on_drop)(self, bufs);
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
            ds.field("num_blocks", &self.num_blocks());
        }
        ds.field("resp", &self.response());
        ds.field("ext", &*self.ext());
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
