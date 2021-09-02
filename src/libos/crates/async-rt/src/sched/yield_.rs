use crate::prelude::*;

pub fn yield_() -> Yield {
    Yield::new()
}

pub struct Yield {
    has_polled: bool,
}

impl Yield {
    pub fn new() -> Self {
        Self { has_polled: false }
    }
}

impl Unpin for Yield {}

impl Future for Yield {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut self_ = self.as_mut();
        if !self_.has_polled {
            self_.has_polled = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(())
        }
    }
}
