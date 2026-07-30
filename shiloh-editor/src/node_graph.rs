//! Visual node graph (Blueprints-style flow) for the editor.
//!
//! Nodes and links are editor-local; runtime execution hooks land later.
//! Fully interactive today: create, drag, connect, pan, delete.

use egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use shiloh_scripting::{VisualGraph, VisualLink, VisualNode, VisualNodeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Event,
    Action,
    Math,
    Query,
}

impl NodeKind {
    pub fn color(self) -> Color32 {
        match self {
            Self::Event => Color32::from_rgb(200, 72, 72),
            Self::Action => Color32::from_rgb(64, 132, 240),
            Self::Math => Color32::from_rgb(72, 176, 108),
            Self::Query => Color32::from_rgb(196, 156, 52),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: u64,
    pub kind: NodeKind,
    pub title: String,
    pub pos: Pos2,
    pub inputs: Vec<&'static str>,
    pub outputs: Vec<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphLink {
    pub from_node: u64,
    pub from_pin: usize,
    pub to_node: u64,
    pub to_pin: usize,
}

#[derive(Debug, Default)]
pub struct NodeGraph {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
    next_id: u64,
    pan: Vec2,
    dragging: Option<(u64, Vec2)>,
    link_drag: Option<(u64, usize)>,
    selected: Option<u64>,
}

impl NodeGraph {
    pub fn new_demo() -> Self {
        let mut g = Self::default();
        g.add_node(
            NodeKind::Event,
            "On Begin Play",
            Pos2::new(40.0, 80.0),
            &[],
            &["Exec"],
        );
        g.add_node(
            NodeKind::Query,
            "Get Player",
            Pos2::new(260.0, 40.0),
            &["Exec"],
            &["Exec", "Entity"],
        );
        g.add_node(
            NodeKind::Action,
            "Play Sound",
            Pos2::new(520.0, 100.0),
            &["Exec", "Sound"],
            &["Exec"],
        );
        g.add_node(
            NodeKind::Math,
            "Float +",
            Pos2::new(260.0, 220.0),
            &["A", "B"],
            &["Sum"],
        );
        if g.nodes.len() >= 3 {
            g.links.push(GraphLink {
                from_node: g.nodes[0].id,
                from_pin: 0,
                to_node: g.nodes[1].id,
                to_pin: 0,
            });
            g.links.push(GraphLink {
                from_node: g.nodes[1].id,
                from_pin: 0,
                to_node: g.nodes[2].id,
                to_pin: 0,
            });
        }
        g
    }

    pub fn add_node(
        &mut self,
        kind: NodeKind,
        title: impl Into<String>,
        pos: Pos2,
        inputs: &[&'static str],
        outputs: &[&'static str],
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.nodes.push(GraphNode {
            id,
            kind,
            title: title.into(),
            pos,
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
        });
        id
    }

    /// Serialize to Phase 4 visual scripting IR.
    pub fn to_visual_graph(&self, name: impl Into<String>) -> VisualGraph {
        VisualGraph {
            name: name.into(),
            nodes: self
                .nodes
                .iter()
                .map(|n| VisualNode {
                    id: n.id,
                    kind: match n.kind {
                        NodeKind::Event => VisualNodeKind::Event,
                        NodeKind::Action => VisualNodeKind::Action,
                        NodeKind::Math => VisualNodeKind::Math,
                        NodeKind::Query => VisualNodeKind::Query,
                    },
                    title: n.title.clone(),
                    pos: [n.pos.x, n.pos.y],
                    inputs: n.inputs.iter().map(|s| (*s).to_string()).collect(),
                    outputs: n.outputs.iter().map(|s| (*s).to_string()).collect(),
                })
                .collect(),
            links: self
                .links
                .iter()
                .map(|l| VisualLink {
                    from_node: l.from_node,
                    from_pin: l.from_pin,
                    to_node: l.to_node,
                    to_pin: l.to_pin,
                })
                .collect(),
        }
    }

    fn node_mut(&mut self, id: u64) -> Option<&mut GraphNode> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    fn node(&self, id: u64) -> Option<&GraphNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    fn screen_pos(&self, rect: Rect, graph: Pos2) -> Pos2 {
        rect.min + self.pan + graph.to_vec2()
    }

    fn output_pin(&self, rect: Rect, node: &GraphNode, index: usize) -> Pos2 {
        self.screen_pos(
            rect,
            Pos2::new(node.pos.x + 180.0, node.pos.y + 36.0 + index as f32 * 22.0),
        )
    }

    fn input_pin(&self, rect: Rect, node: &GraphNode, index: usize) -> Pos2 {
        self.screen_pos(
            rect,
            Pos2::new(node.pos.x, node.pos.y + 36.0 + index as f32 * 22.0),
        )
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.strong("Node Graph");
            ui.separator();
            if ui.button("+ Event").clicked() {
                let pos = Pos2::new(80.0 - self.pan.x, 80.0 - self.pan.y);
                self.add_node(NodeKind::Event, "Custom Event", pos, &[], &["Exec"]);
            }
            if ui.button("+ Action").clicked() {
                let pos = Pos2::new(120.0 - self.pan.x, 140.0 - self.pan.y);
                self.add_node(NodeKind::Action, "Action", pos, &["Exec"], &["Exec"]);
            }
            if ui.button("+ Math").clicked() {
                let pos = Pos2::new(160.0 - self.pan.x, 200.0 - self.pan.y);
                self.add_node(NodeKind::Math, "Float +", pos, &["A", "B"], &["Sum"]);
            }
            if ui.button("+ Query").clicked() {
                let pos = Pos2::new(200.0 - self.pan.x, 160.0 - self.pan.y);
                self.add_node(
                    NodeKind::Query,
                    "Get Entity",
                    pos,
                    &["Exec"],
                    &["Exec", "Entity"],
                );
            }
            if ui.button("Clear links").clicked() {
                self.links.clear();
            }
            if self.selected.is_some() && ui.button("Delete node").clicked() {
                let id = self.selected.unwrap();
                self.nodes.retain(|n| n.id != id);
                self.links
                    .retain(|l| l.from_node != id && l.to_node != id);
                self.selected = None;
            }
        });

        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;

        painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 20, 26));
        let grid = 24.0;
        let stroke = Stroke::new(1.0_f32, Color32::from_rgb(32, 36, 46));
        let ox = self.pan.x.rem_euclid(grid);
        let mut x = ox;
        while x < rect.width() {
            let px = rect.left() + x;
            painter.line_segment(
                [Pos2::new(px, rect.top()), Pos2::new(px, rect.bottom())],
                stroke,
            );
            x += grid;
        }
        let oy = self.pan.y.rem_euclid(grid);
        let mut y = oy;
        while y < rect.height() {
            let py = rect.top() + y;
            painter.line_segment(
                [Pos2::new(rect.left(), py), Pos2::new(rect.right(), py)],
                stroke,
            );
            y += grid;
        }

        if resp.dragged_by(egui::PointerButton::Middle)
            || resp.dragged_by(egui::PointerButton::Secondary)
        {
            self.pan += resp.drag_delta();
        }

        for link in &self.links {
            let Some(a) = self.node(link.from_node) else {
                continue;
            };
            let Some(b) = self.node(link.to_node) else {
                continue;
            };
            let from = self.output_pin(rect, a, link.from_pin);
            let to = self.input_pin(rect, b, link.to_pin);
            let c1 = from + Vec2::new(70.0, 0.0);
            let c2 = to - Vec2::new(70.0, 0.0);
            painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
                [from, c1, c2, to],
                false,
                Color32::TRANSPARENT,
                Stroke::new(2.0_f32, Color32::from_rgb(120, 170, 255)),
            ));
        }

        if let Some((nid, pin)) = self.link_drag
            && let Some(a) = self.node(nid)
            && let Some(mouse) = resp.interact_pointer_pos()
        {
            let from = self.output_pin(rect, a, pin);
            painter.line_segment(
                [from, mouse],
                Stroke::new(2.0_f32, Color32::LIGHT_BLUE),
            );
        }

        let pointer = resp.interact_pointer_pos();
        let mut start_link: Option<(u64, usize)> = None;
        let mut finish_link: Option<(u64, usize)> = None;
        let mut clicked_node: Option<u64> = None;
        let mut start_drag: Option<(u64, Vec2)> = None;

        for node in &self.nodes {
            let pos = self.screen_pos(rect, node.pos);
            let rows = node.inputs.len().max(node.outputs.len()).max(1);
            let size = Vec2::new(180.0, 40.0 + rows as f32 * 22.0);
            let node_rect = Rect::from_min_size(pos, size);
            let selected = self.selected == Some(node.id);

            painter.rect_filled(node_rect, 6.0, Color32::from_rgb(28, 32, 42));
            painter.rect_stroke(
                node_rect,
                6.0,
                Stroke::new(
                    if selected { 2.0_f32 } else { 1.0_f32 },
                    if selected {
                        Color32::from_rgb(80, 160, 255)
                    } else {
                        Color32::from_rgb(50, 56, 70)
                    },
                ),
                egui::StrokeKind::Outside,
            );

            let header = Rect::from_min_size(pos, Vec2::new(size.x, 26.0));
            painter.rect_filled(header, 6.0, node.kind.color().gamma_multiply(0.65));
            painter.text(
                header.left_center() + Vec2::new(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &node.title,
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            );

            for (i, name) in node.inputs.iter().enumerate() {
                let p = self.input_pin(rect, node, i);
                painter.circle_filled(p, 5.0, Color32::from_rgb(180, 190, 210));
                painter.text(
                    p + Vec2::new(10.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    *name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(190, 195, 210),
                );
                if let Some(mp) = pointer
                    && resp.drag_stopped()
                    && (mp - p).length() < 12.0
                {
                    finish_link = Some((node.id, i));
                }
            }
            for (i, name) in node.outputs.iter().enumerate() {
                let p = self.output_pin(rect, node, i);
                painter.circle_filled(p, 5.0, Color32::from_rgb(120, 180, 255));
                painter.text(
                    p + Vec2::new(-10.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    *name,
                    egui::FontId::proportional(11.0),
                    Color32::from_rgb(190, 195, 210),
                );
                if let Some(mp) = pointer
                    && resp.drag_started_by(egui::PointerButton::Primary)
                    && (mp - p).length() < 10.0
                {
                    start_link = Some((node.id, i));
                }
            }

            if let Some(mp) = pointer
                && node_rect.contains(mp)
            {
                if resp.clicked() {
                    clicked_node = Some(node.id);
                }
                if resp.drag_started_by(egui::PointerButton::Primary)
                    && header.contains(mp)
                    && start_link.is_none()
                {
                    start_drag = Some((node.id, mp - pos));
                }
            }
        }

        if let Some(id) = clicked_node {
            self.selected = Some(id);
        }
        if let Some(pair) = start_drag {
            self.dragging = Some(pair);
            self.selected = Some(pair.0);
        }
        if let Some((id, pin)) = start_link {
            self.link_drag = Some((id, pin));
            self.dragging = None;
        }

        if let Some((id, grab)) = self.dragging
            && let Some(mp) = pointer
        {
            let pan = self.pan;
            if let Some(node) = self.node_mut(id) {
                node.pos = (mp - rect.min - pan - grab).to_pos2();
            }
        }

        if resp.drag_stopped() {
            if let (Some((from, fpin)), Some((to, tpin))) = (self.link_drag, finish_link)
                && from != to
            {
                self.links.retain(|l| !(l.to_node == to && l.to_pin == tpin));
                self.links.push(GraphLink {
                    from_node: from,
                    from_pin: fpin,
                    to_node: to,
                    to_pin: tpin,
                });
            }
            self.link_drag = None;
            self.dragging = None;
        }
    }
}
