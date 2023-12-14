use std::mem::{self};

use sgx_untrusted_alloc::UntrustedBox;

/// A circular buffer in untrusted memory.
pub struct UntrustedCircularBuf {
    // The underlying storage of the buffer.
    //
    // Note that to differentiate between the state of being full and empty,
    // the actual capacity has to be `buf.len() - 1`
    buf: UntrustedBox<[u8]>,
    // The head of the buf, manipulated by consumer methods.
    //
    // Invariant: 0 <= head < len.
    head: usize,
    // The tail of the buf, manipulated by producer methods.
    //
    // Invariant: 0 <= tail < len.
    tail: usize, // producer
}

impl UntrustedCircularBuf {
    /// Construct a circular buffer.
    pub fn with_capacity(capacity: usize) -> Self {
        debug_assert!(capacity > 0);
        Self {
            buf: UntrustedBox::new_uninit_slice(capacity),
            head: 0,
            tail: 0,
        }
    }

    /// Produce some bytes.
    pub fn produce(&mut self, buf: &[u8]) -> usize {
        self.with_producer_view(|part0, part1| {
            if buf.len() <= part0.len() {
                part0[..buf.len()].copy_from_slice(buf);
                return buf.len();
            }

            part0.copy_from_slice(&buf[..part0.len()]);

            let buf = &buf[part0.len()..];
            if buf.len() <= part1.len() {
                part1[..buf.len()].copy_from_slice(buf);
                return part0.len() + buf.len();
            } else {
                part1.copy_from_slice(&buf[..part1.len()]);
                return part0.len() + part1.len();
            }
        })
    }

    pub fn produce_without_copy(&mut self, len: usize) -> usize {
        self.with_producer_view(|part0, part1| {
            // println!("part0: {}, part1: {}, produce len: {}", part0.len(), part1.len(), len);
            len.min(part0.len() + part1.len())
        })
    }

    pub fn producible(&self) -> usize {
        self.capacity() - self.consumable()
    }

    pub fn with_producer_view(&mut self, f: impl FnOnce(&mut [u8], &mut [u8]) -> usize) -> usize {
        let head = self.head;
        let tail = self.tail;
        let len = self.buf.len();

        let (range0, range1) = if tail >= head {
            if head > 0 {
                (tail..len, 0..(head - 1))
            } else if tail < len - 1 {
                (tail..(len - 1), 0..0)
            } else {
                (0..0, 0..0)
            }
        } else if tail < head - 1 {
            (tail..(head - 1), 0..0)
        } else {
            (0..0, 0..0)
        };
        // To reason about the above two resulting ranges, here is two figures that
        // illustrate two typical settings.
        //
        // Setting 1:
        //
        // indexes:     0 1 2 3     ...     L-1
        // bytes:      [ | |*|*|*|*|*| | | | ]
        //                 ^         ^
        // cursors:        head      tail
        //
        // Setting 2:
        //
        // indexes:     0 1 2 3     ...     L-1
        // bytes:      [*|*|*|*| | | |*|*|*|*]
        //                     ^     ^
        // cursors:            tail  head
        //
        // where L = self.len and "*" indicates a stored byte.

        // Safety. It is ok to acquire two mutable subslices from the buf since the two
        // subslices are guaranteed to be exclusive to each other.
        let (part0, part1) = unsafe {
            #![allow(mutable_transmutes)]
            let part0 = mem::transmute::<&[u8], &mut [u8]>(&self.buf[range0]);
            let part1 = mem::transmute::<&[u8], &mut [u8]>(&self.buf[range1]);
            (part0, part1)
        };

        let bytes_produced = f(part0, part1);
        assert!(bytes_produced <= self.producible());

        self.tail = (tail + bytes_produced) % len;
        bytes_produced
    }

    pub fn consume(&mut self, buf: &mut [u8]) -> usize {
        self.with_consumer_view(|part0, part1| {
            if buf.len() <= part0.len() {
                buf.copy_from_slice(&part0[..buf.len()]);
                return buf.len();
            }

            buf[..part0.len()].copy_from_slice(part0);

            let buf = &mut buf[part0.len()..];
            if buf.len() <= part1.len() {
                buf.copy_from_slice(&part1[..buf.len()]);
                return part0.len() + buf.len();
            } else {
                buf[..part1.len()].copy_from_slice(part1);
                return part0.len() + part1.len();
            }
        })
    }

    pub fn consume_without_copy(&mut self, len: usize) -> usize {
        self.with_consumer_view(|part0, part1| len.min(part0.len() + part1.len()))
    }

    pub fn with_consumer_view(&mut self, f: impl FnOnce(&[u8], &[u8]) -> usize) -> usize {
        let head = self.head;
        let tail = self.tail;
        let len = self.buf.len();

        let (range0, range1) = if head <= tail {
            (head..tail, 0..0)
        } else {
            (head..len, 0..tail)
        };

        let part0 = &self.buf[range0];
        let part1 = &self.buf[range1];

        let bytes_consumed = f(part0, part1);
        assert!(bytes_consumed <= self.consumable());

        self.head = (head + bytes_consumed) % len;
        bytes_consumed
    }

    pub fn consumable(&self) -> usize {
        let head = self.head;
        let tail = self.tail;
        let len = self.buf.len();

        if head <= tail {
            tail - head
        } else {
            (len - head) + tail
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len() - 1
    }

    pub fn is_full(&self) -> bool {
        self.producible() == 0
    }

    pub fn is_empty(&self) -> bool {
        self.consumable() == 0
    }

    /// Returns a slice that contains the entire buffer.
    #[allow(dead_code)]
    pub fn as_slice(&self) -> &[u8] {
        &*self.buf
    }

    /// Returns a mutable slice that contains the entire buffer.
    #[allow(dead_code)]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut *self.buf
    }
}

impl std::fmt::Debug for UntrustedCircularBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UntrustedCircularBuf")
            .field("capacity", &self.capacity())
            .field("producible", &self.producible())
            .field("consumable", &self.consumable())
            .finish()
    }
}
