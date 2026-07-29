//! Generational handles — stable IDs that detect use-after-free without raw pointers.
//!
//! Industry pattern (SlotMap / Bevy Entity / Flecs IDs): pack index + generation into a `u64`.

use core::fmt;
use core::marker::PhantomData;
use core::num::NonZeroU32;

/// Strongly typed generational handle.
///
/// `T` is a phantom tag so `Handle<Mesh>` cannot be passed where `Handle<Texture>` is expected.
#[repr(transparent)]
pub struct Handle<T> {
    raw: u64,
    _tag: PhantomData<fn() -> T>,
}

// Manual Copy/Clone: PhantomData should not impose `T: Copy`.
impl<T> Copy for Handle<T> {}
impl<T> Clone for Handle<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> PartialEq for Handle<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}
impl<T> Eq for Handle<T> {}

impl<T> core::hash::Hash for Handle<T> {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

impl<T> Handle<T> {
    /// Creates a handle from a raw packed value. Prefer [`HandleAllocator`].
    #[inline]
    pub const fn from_raw(raw: u64) -> Self {
        Self {
            raw,
            _tag: PhantomData,
        }
    }

    /// Packs index and generation into a handle.
    #[inline]
    pub const fn new(index: u32, generation: NonZeroU32) -> Self {
        Self::from_raw(((generation.get() as u64) << 32) | index as u64)
    }

    #[inline]
    pub const fn raw(self) -> u64 {
        self.raw
    }

    #[inline]
    pub const fn index(self) -> u32 {
        self.raw as u32
    }

    #[inline]
    pub const fn generation(self) -> u32 {
        (self.raw >> 32) as u32
    }

    /// Reinterpret tag without changing the packed bits (e.g. asset type erasure).
    #[inline]
    pub const fn cast<U>(self) -> Handle<U> {
        Handle::from_raw(self.raw)
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Handle<{}>({}, gen {})",
            core::any::type_name::<T>(),
            self.index(),
            self.generation()
        )
    }
}

impl<T> Default for Handle<T> {
    fn default() -> Self {
        // Generation 0 is reserved as "null" / never allocated.
        Self::from_raw(0)
    }
}

/// Recycles free indices; bumps generation on recycle to invalidate stale handles.
#[derive(Debug, Default)]
pub struct HandleAllocator<T> {
    generations: Vec<u32>,
    free: Vec<u32>,
    _tag: PhantomData<fn() -> T>,
}

impl<T> HandleAllocator<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            generations: Vec::new(),
            free: Vec::new(),
            _tag: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            generations: Vec::with_capacity(capacity),
            free: Vec::new(),
            _tag: PhantomData,
        }
    }

    /// Allocates a live handle.
    pub fn alloc(&mut self) -> Handle<T> {
        if let Some(index) = self.free.pop() {
            let generation_value = self.generations[index as usize];
            let generation =
                NonZeroU32::new(generation_value).expect("generation must be non-zero while live");
            Handle::new(index, generation)
        } else {
            let index = u32::try_from(self.generations.len()).expect("handle index overflow");
            self.generations.push(1);
            Handle::new(index, NonZeroU32::new(1).unwrap())
        }
    }

    /// Frees a handle. Returns `false` if the handle was already stale.
    pub fn free(&mut self, handle: Handle<T>) -> bool {
        let index = handle.index() as usize;
        if index >= self.generations.len() {
            return false;
        }
        if self.generations[index] != handle.generation() {
            return false;
        }
        // Bump generation; wrap to 1 (0 stays reserved as null).
        let next = handle.generation().wrapping_add(1).max(1);
        self.generations[index] = next;
        self.free.push(handle.index());
        true
    }

    #[inline]
    pub fn is_live(&self, handle: Handle<T>) -> bool {
        let index = handle.index() as usize;
        index < self.generations.len() && self.generations[index] == handle.generation()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.generations.len().saturating_sub(self.free.len())
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mesh;

    #[test]
    fn recycle_invalidates_old_handle() {
        let mut alloc = HandleAllocator::<Mesh>::new();
        let a = alloc.alloc();
        assert!(alloc.free(a));
        let b = alloc.alloc();
        assert_eq!(a.index(), b.index());
        assert_ne!(a.generation(), b.generation());
        assert!(!alloc.is_live(a));
        assert!(alloc.is_live(b));
    }
}
