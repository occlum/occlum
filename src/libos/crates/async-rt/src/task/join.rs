use alloc::sync::Weak;
use core::marker::PhantomData;
use core::task::{Context, Poll, Waker};

use crate::prelude::*;

pub fn new<T: Send + 'static>() -> (JoinHandle<T>, OutputHandle<T>) {
    let state = Arc::new(Mutex::new(State::new()));
    let output_handle = OutputHandle {
        state: Arc::downgrade(&state),
        phantom: PhantomData,
    };
    let join_handle = JoinHandle {
        state: state,
        phantom: PhantomData,
    };
    (join_handle, output_handle)
}

pub struct JoinHandle<T: Send + 'static> {
    state: Arc<Mutex<State<T>>>,
    phantom: PhantomData<T>,
}

impl<T: Send + 'static> Unpin for JoinHandle<T> {}

impl<T: Send + 'static> Future for JoinHandle<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut state = self.state.lock();
        if let Some(output) = state.take_output(cx) {
            Poll::Ready(output)
        } else {
            Poll::Pending
        }
    }
}

pub struct OutputHandle<T: Send + 'static> {
    state: Weak<Mutex<State<T>>>,
    phantom: PhantomData<T>,
}

impl<T: Send + 'static> OutputHandle<T> {
    pub fn set(self, output: T) {
        if let Some(state) = self.state.upgrade() {
            let mut state = state.lock();
            state.set_output(output);
        }
    }
}

// The state of a task that is to be joined.
#[derive(Debug)]
enum State<T: Send + 'static> {
    Init,
    Pending(Waker),
    Ready(T),
    Finish,
}

impl<T: Send + 'static> State<T> {
    pub fn new() -> Self {
        State::Init
    }

    pub fn set_output(&mut self, value: T) {
        *self = match self {
            State::Init => State::Ready(value),
            State::Pending(waker) => {
                waker.wake_by_ref();
                State::Ready(value)
            }
            State::Ready(_) | State::Finish => {
                panic!("a task's output must not be set twice");
            }
        };
    }

    pub fn take_output(&mut self, cx: &mut Context<'_>) -> Option<T> {
        match self {
            State::Init | State::Pending(_) => {
                *self = State::Pending(cx.waker().clone());
                None
            }
            State::Ready(value) => {
                drop(value);
                let mut result = State::Finish;
                core::mem::swap(self, &mut result);
                if let State::Ready(value) = result {
                    Some(value)
                } else {
                    unreachable!();
                }
            }
            State::Finish => {
                panic!("a task's output must not be taken again");
            }
        }
    }
}
