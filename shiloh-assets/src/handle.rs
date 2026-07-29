//! Typed asset handles via core generational IDs.

use shiloh_core::Handle;

#[derive(Debug, Clone, Copy, Default)]
pub struct AssetTag;

pub type AssetId = Handle<AssetTag>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetState {
    Loading,
    Ready,
    Failed,
}
