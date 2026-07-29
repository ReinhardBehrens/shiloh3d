//! Transient render graph — declare passes + resources, compile to execution order.
//!
//! Classic Frostbite / modern engine technique: virtual resources, lifetime tracking,
//! topological execution.

use ahash::AHashMap;
use smallvec::SmallVec;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResourceId(pub u32);

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("cycle detected in render graph")]
    Cycle,
    #[error("unknown resource {0:?}")]
    UnknownResource(ResourceId),
}

pub struct PassNode {
    pub name: &'static str,
    pub reads: SmallVec<[ResourceId; 4]>,
    pub writes: SmallVec<[ResourceId; 4]>,
    pub execute: Box<dyn FnMut() + Send>,
}

/// Builder + compiled DAG of GPU passes.
#[derive(Default)]
pub struct RenderGraph {
    next_resource: u32,
    resources: AHashMap<ResourceId, &'static str>,
    passes: Vec<PassNode>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_resource(&mut self, name: &'static str) -> ResourceId {
        let id = ResourceId(self.next_resource);
        self.next_resource = self.next_resource.wrapping_add(1);
        self.resources.insert(id, name);
        id
    }

    pub fn add_pass(&mut self, pass: PassNode) {
        self.passes.push(pass);
    }

    /// Kahn topological sort over pass write→read edges.
    pub fn compile_order(&self) -> Result<Vec<usize>, GraphError> {
        let n = self.passes.len();
        let mut indegree = vec![0u32; n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];

        // Resource writer → later readers.
        let mut writer: AHashMap<ResourceId, usize> = AHashMap::new();
        for (i, pass) in self.passes.iter().enumerate() {
            for &w in &pass.writes {
                if !self.resources.contains_key(&w) {
                    return Err(GraphError::UnknownResource(w));
                }
                writer.insert(w, i);
            }
        }
        for (i, pass) in self.passes.iter().enumerate() {
            for &r in &pass.reads {
                if let Some(&w) = writer.get(&r)
                    && w != i
                {
                    adj[w].push(i);
                    indegree[i] += 1;
                }
            }
        }

        let mut queue: Vec<usize> = indegree
            .iter()
            .enumerate()
            .filter_map(|(i, &d)| (d == 0).then_some(i))
            .collect();
        let mut order = Vec::with_capacity(n);
        while let Some(i) = queue.pop() {
            order.push(i);
            for &j in &adj[i] {
                indegree[j] -= 1;
                if indegree[j] == 0 {
                    queue.push(j);
                }
            }
        }
        if order.len() != n {
            return Err(GraphError::Cycle);
        }
        Ok(order)
    }

    pub fn execute(&mut self) -> Result<(), GraphError> {
        let order = self.compile_order()?;
        for idx in order {
            (self.passes[idx].execute)();
        }
        Ok(())
    }

    pub fn clear(&mut self) {
        self.passes.clear();
        self.resources.clear();
        self.next_resource = 0;
    }
}
