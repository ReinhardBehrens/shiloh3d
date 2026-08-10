//! Visual scripting graph IR (Phase 4) — editor node graphs serialize here.
//!
//! Minimal interpreter: walk Event → linked Action/Query/Math nodes and collect
//! executed titles. Full gameplay VM comes later.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualNodeKind {
    Event,
    Action,
    Math,
    Query,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualNode {
    pub id: u64,
    pub kind: VisualNodeKind,
    pub title: String,
    pub pos: [f32; 2],
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualLink {
    pub from_node: u64,
    pub from_pin: usize,
    pub to_node: u64,
    pub to_pin: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VisualGraph {
    pub name: String,
    pub nodes: Vec<VisualNode>,
    pub links: Vec<VisualLink>,
}

/// One step recorded by [`VisualGraph::execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualExecStep {
    pub node_id: u64,
    pub title: String,
    pub kind: VisualNodeKind,
}

/// Coarse action kinds inferred from Action node titles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualAction {
    Spawn,
    SetTranslation,
    EmitSignal,
    PlayAudio,
    Nop,
}

/// Map executed Action steps to [`VisualAction`]s by title keywords.
pub fn actions_from_steps(steps: &[VisualExecStep]) -> Vec<VisualAction> {
    steps
        .iter()
        .filter(|s| s.kind == VisualNodeKind::Action)
        .map(|s| {
            let t = s.title.to_ascii_lowercase();
            if t.contains("spawn") {
                VisualAction::Spawn
            } else if t.contains("translat")
                || t.split(|c: char| !c.is_ascii_alphanumeric())
                    .any(|w| w == "move")
            {
                // "translat" matches both Translate and Set Translation.
                VisualAction::SetTranslation
            } else if t.contains("signal") {
                VisualAction::EmitSignal
            } else if t.contains("audio") || t.contains("sound") {
                VisualAction::PlayAudio
            } else {
                VisualAction::Nop
            }
        })
        .collect()
}

impl VisualGraph {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            links: Vec::new(),
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    pub fn save(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        let text = self
            .to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)
    }

    fn node(&self, id: u64) -> Option<&VisualNode> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// Fire the first Event node (or `event_title` match) and follow Exec-style links.
    pub fn execute(&self, event_title: Option<&str>) -> Vec<VisualExecStep> {
        let start = self.nodes.iter().find(|n| {
            n.kind == VisualNodeKind::Event
                && event_title
                    .map(|t| n.title == t)
                    .unwrap_or(true)
        });
        let Some(start) = start else {
            return Vec::new();
        };

        let mut steps = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start.id);

        while let Some(id) = queue.pop_front() {
            if !visited.insert(id) {
                continue;
            }
            let Some(node) = self.node(id) else {
                continue;
            };
            steps.push(VisualExecStep {
                node_id: node.id,
                title: node.title.clone(),
                kind: node.kind,
            });
            for link in self.links.iter().filter(|l| l.from_node == id) {
                queue.push_back(link.to_node);
            }
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn executes_event_chain() {
        let g = VisualGraph {
            name: "t".into(),
            nodes: vec![
                VisualNode {
                    id: 1,
                    kind: VisualNodeKind::Event,
                    title: "On Begin Play".into(),
                    pos: [0.0, 0.0],
                    inputs: vec![],
                    outputs: vec!["Exec".into()],
                },
                VisualNode {
                    id: 2,
                    kind: VisualNodeKind::Action,
                    title: "Play Sound".into(),
                    pos: [1.0, 0.0],
                    inputs: vec!["Exec".into()],
                    outputs: vec!["Exec".into()],
                },
            ],
            links: vec![VisualLink {
                from_node: 1,
                from_pin: 0,
                to_node: 2,
                to_pin: 0,
            }],
        };
        let steps = g.execute(Some("On Begin Play"));
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].title, "Play Sound");
        assert_eq!(
            actions_from_steps(&steps),
            vec![VisualAction::PlayAudio]
        );
    }

    #[test]
    fn actions_from_titles() {
        let steps = vec![
            VisualExecStep {
                node_id: 1,
                title: "On Begin Play".into(),
                kind: VisualNodeKind::Event,
            },
            VisualExecStep {
                node_id: 2,
                title: "Spawn Actor".into(),
                kind: VisualNodeKind::Action,
            },
            VisualExecStep {
                node_id: 3,
                title: "Set Translation".into(),
                kind: VisualNodeKind::Action,
            },
            VisualExecStep {
                node_id: 4,
                title: "Move To".into(),
                kind: VisualNodeKind::Action,
            },
            VisualExecStep {
                node_id: 5,
                title: "Emit Signal".into(),
                kind: VisualNodeKind::Action,
            },
            VisualExecStep {
                node_id: 6,
                title: "Play Audio Clip".into(),
                kind: VisualNodeKind::Action,
            },
            VisualExecStep {
                node_id: 7,
                title: "Custom Thing".into(),
                kind: VisualNodeKind::Action,
            },
        ];
        assert_eq!(
            actions_from_steps(&steps),
            vec![
                VisualAction::Spawn,
                VisualAction::SetTranslation,
                VisualAction::SetTranslation,
                VisualAction::EmitSignal,
                VisualAction::PlayAudio,
                VisualAction::Nop,
            ]
        );
    }
}
