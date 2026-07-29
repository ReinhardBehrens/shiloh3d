//! Per-frame scratch storage — allocate temporary data without long-lived heap churn.
//!
//! Fully safe Rust. Reset at end of frame (single-threaded ownership).

use core::cell::RefCell;

/// Byte bump buffer for ephemeral frame data.
#[derive(Debug)]
pub struct FrameAllocator {
    buffer: RefCell<Vec<u8>>,
    used: RefCell<usize>,
}

impl FrameAllocator {
    pub fn with_capacity(bytes: usize) -> Self {
        Self {
            buffer: RefCell::new(vec![0u8; bytes]),
            used: RefCell::new(0),
        }
    }

    #[inline]
    pub fn reset(&self) {
        *self.used.borrow_mut() = 0;
    }

    #[inline]
    pub fn used(&self) -> usize {
        *self.used.borrow()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.borrow().len()
    }

    /// Reserves `len` bytes for this frame. Returns start offset, or `None` if OOM.
    pub fn alloc_bytes(&self, len: usize) -> Option<usize> {
        let mut used = self.used.borrow_mut();
        let start = *used;
        let end = start.checked_add(len)?;
        if end > self.buffer.borrow().len() {
            return None;
        }
        *used = end;
        Some(start)
    }

    /// Writes `data` into the bump buffer; returns the offset.
    pub fn write_bytes(&self, data: &[u8]) -> Option<usize> {
        let start = self.alloc_bytes(data.len())?;
        self.buffer.borrow_mut()[start..start + data.len()].copy_from_slice(data);
        Some(start)
    }

    /// Copies a POD-like byte view of `items` (caller serializes).
    pub fn write_slice_bytes<T>(&self, items: &[T], encode: impl Fn(&[T], &mut [u8])) -> Option<usize> {
        let byte_len = core::mem::size_of_val(items);
        let start = self.alloc_bytes(byte_len)?;
        let mut buf = self.buffer.borrow_mut();
        encode(items, &mut buf[start..start + byte_len]);
        Some(start)
    }
}
