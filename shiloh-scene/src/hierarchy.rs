//! Parent/child hierarchy components.

use shiloh_ecs::Entity;
use smallvec::SmallVec;

#[derive(Debug, Clone, Copy)]
pub struct Parent(pub Entity);

#[derive(Debug, Clone, Default)]
pub struct Children(pub SmallVec<[Entity; 4]>);
