//! Parent/child hierarchy components and world-space propagation.

use ahash::{AHashMap, AHashSet};
use glam::Mat4;
use shiloh_ecs::{Entity, World};
use smallvec::SmallVec;

use crate::transform::{GlobalTransform, Transform};

#[derive(Debug, Clone, Copy)]
pub struct Parent(pub Entity);

#[derive(Debug, Clone, Default)]
pub struct Children(pub SmallVec<[Entity; 4]>);

/// Attach `child` under `parent`, updating both `Parent` and `Children`.
pub fn set_parent(world: &mut World, child: Entity, parent: Entity) {
    if let Some(Parent(old)) = world.get::<Parent>(child).copied()
        && let Some(children) = world.get_mut::<Children>(old)
    {
        children.0.retain(|e| *e != child);
    }

    let _ = world.insert(child, Parent(parent));
    if let Some(children) = world.get_mut::<Children>(parent) {
        if !children.0.contains(&child) {
            children.0.push(child);
        }
    } else {
        let mut c = Children::default();
        c.0.push(child);
        let _ = world.insert(parent, c);
    }

    if let Some(t) = world.get_mut::<Transform>(child) {
        t.mark_dirty();
    }
}

/// Recompute `GlobalTransform` for every entity with a `Transform`.
///
/// Roots (no `Parent`) use local matrices; children multiply by the parent's
/// global matrix. Dirty flags are cleared after a successful update.
pub fn propagate_transforms(world: &mut World) {
    // Collect entities + parent links first (avoid borrow clashes).
    let mut entities: Vec<Entity> = Vec::new();
    world.for_each::<Transform>(|e, _| entities.push(e));

    let parents: AHashMap<Entity, Entity> = entities
        .iter()
        .filter_map(|&e| world.get::<Parent>(e).map(|p| (e, p.0)))
        .collect();

    // Ensure GlobalTransform exists.
    for &e in &entities {
        if world.get::<GlobalTransform>(e).is_none() {
            let _ = world.insert(e, GlobalTransform::default());
        }
    }

    // Topological-ish: process roots then walk by depth using a worklist.
    let mut done: AHashSet<Entity> = AHashSet::new();
    let mut pending = entities.clone();
    let mut guard = 0usize;
    while !pending.is_empty() && guard < entities.len() + 2 {
        guard += 1;
        let mut next = Vec::new();
        for e in pending {
            let parent = parents.get(&e).copied();
            if let Some(p) = parent
                && !done.contains(&p)
            {
                next.push(e);
                continue;
            }
            let local = world
                .get::<Transform>(e)
                .map(|t| t.matrix())
                .unwrap_or(Mat4::IDENTITY);
            let global = match parent {
                Some(p) => world
                    .get::<GlobalTransform>(p)
                    .map(|g| g.0 * local)
                    .unwrap_or(local),
                None => local,
            };
            if let Some(g) = world.get_mut::<GlobalTransform>(e) {
                g.0 = global;
            }
            if let Some(t) = world.get_mut::<Transform>(e) {
                t.dirty = false;
            }
            done.insert(e);
        }
        pending = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Transform;
    use glam::Vec3;

    #[test]
    fn child_inherits_parent_translation() {
        let mut world = World::new();
        let parent = world.spawn(Transform::from_translation(Vec3::new(10.0, 0.0, 0.0)));
        let child = world.spawn(Transform::from_translation(Vec3::new(1.0, 2.0, 0.0)));
        set_parent(&mut world, child, parent);
        propagate_transforms(&mut world);
        let g = world.get::<GlobalTransform>(child).unwrap().0;
        let t = g.w_axis.truncate();
        assert!((t.x - 11.0).abs() < 1e-4);
        assert!((t.y - 2.0).abs() < 1e-4);
    }
}
