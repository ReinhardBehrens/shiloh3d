//! High-level renderer owning the per-frame graph.

use shiloh_rhi::Device;
use tracing::debug;

use crate::frame::FrameContext;
use crate::graph::{PassNode, RenderGraph};
use smallvec::SmallVec;

pub struct Renderer {
    graph: RenderGraph,
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            graph: RenderGraph::new(),
        }
    }

    pub fn begin_frame(&mut self, ctx: &FrameContext<'_>) {
        self.graph.clear();
        debug!(
            frame = ctx.frame_index,
            backend = ctx.device.info().backend,
            "begin frame"
        );
        let color = self.graph.create_resource("swapchain_color");
        self.graph.add_pass(PassNode {
            name: "clear",
            reads: SmallVec::new(),
            writes: SmallVec::from_elem(color, 1),
            execute: Box::new(|| {}),
        });
    }

    pub fn end_frame(&mut self, device: &dyn Device) -> Result<(), crate::graph::GraphError> {
        self.graph.execute()?;
        device.queue().present();
        Ok(())
    }

    pub fn graph_mut(&mut self) -> &mut RenderGraph {
        &mut self.graph
    }
}
