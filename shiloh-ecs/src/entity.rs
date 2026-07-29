//! Generational entity IDs.

use core::fmt;
use shiloh_core::handle::Handle;

/// Tag for entity handles.
#[derive(Debug, Clone, Copy, Default)]
pub struct EntityTag;

/// Entity = generational handle into the world.
pub type Entity = Handle<EntityTag>;

impl fmt::Display for EntityTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Entity")
    }
}
