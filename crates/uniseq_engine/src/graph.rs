//! Graph data structures: nodes and directed edges derived from a WorkspaceIndex.

use crate::index::WorkspaceIndex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A graph node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord)]
pub struct GraphNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
}

// Manual implementation to order by id only (id is unique)
impl PartialOrd for GraphNode {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.id.cmp(&other.id))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

impl PartialOrd for GraphEdge {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.from.cmp(&other.from).then(self.to.cmp(&other.to)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum NodeKind {
    Page,
    Tag,
    Journal,
    Asset,
}

/// Full graph snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Build graph data from a workspace index and optional asset list.
pub fn graph_data(index: &WorkspaceIndex) -> GraphData {
    let mut nodes: BTreeSet<GraphNode> = BTreeSet::new();
    let mut edges: BTreeSet<GraphEdge> = BTreeSet::new();

    // Journal nodes
    for doc in &index.documents {
        if let crate::model::DocumentKind::Journal { date } = &doc.kind {
            nodes.insert(GraphNode {
                id: format!("journal:{}", date),
                kind: NodeKind::Journal,
                label: date.to_string(),
            });
        }
    }

    // Page nodes + outgoing edges for links and tags
    for (page_path, page) in &index.pages {
        nodes.insert(GraphNode {
            id: format!("page:{}", page_path),
            kind: if page.has_file { NodeKind::Page } else { NodeKind::Tag },
            label: page_path.clone(),
        });
        for target in &page.outbound_pages {
            let edge = GraphEdge {
                from: format!("page:{}", page_path),
                to: format!("page:{}", target),
                label: None,
            };
            edges.insert(edge);
        }
        // Edges from journal entries in this page's file
        for entry in &page.own_entry_ids {
            if let Some(e) = index.entries.iter().find(|e| &e.runtime_id == entry) {
                if let Some(d) = index.documents.iter().find(|d| &d.path == &e.anchor.file_path) {
                    if let crate::model::DocumentKind::Journal { date } = &d.kind {
                        edges.insert(GraphEdge {
                            from: format!("journal:{}", date),
                            to: format!("page:{}", page_path),
                            label: None,
                        });
                    }
                }
                for link in &e.links {
                    edges.insert(GraphEdge {
                        from: format!("page:{}", page_path),
                        to: format!("page:{}", link.page_path),
                        label: Some("link".into()),
                    });
                }
                for tag in &e.tags {
                    edges.insert(GraphEdge {
                        from: format!("page:{}", page_path),
                        to: format!("tag:{}", tag.page_path),
                        label: Some("tag".into()),
                    });
                    nodes.insert(GraphNode {
                        id: format!("tag:{}", tag.page_path),
                        kind: NodeKind::Tag,
                        label: tag.page_path.clone(),
                    });
                }
            }
        }
    }

    let nodes = nodes.into_iter().collect();
    let edges = edges.into_iter().collect();
    GraphData { nodes, edges }
}