//! Metadata-only, explicit-link graph indexing for Kairos.
//!
//! This module deliberately does **not** discover files. Callers must first
//! apply Kairos read/privacy policy and pass only exact, approved Markdown
//! paths in [`GraphBrainSource::allowlisted_markdown_paths`]. The scanner
//! rejects protected paths before it resolves or reads them, and it retains no
//! note body or excerpt in the resulting [`GraphIndex`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// The stable ID of the central Kairos node.
pub const KAIROS_NODE_ID: &str = "kairos";

/// The graph size used unless the caller deliberately chooses a smaller cap.
pub const DEFAULT_GRAPH_NODE_CAP: usize = 500;

/// A hard upper bound which keeps a whole-brain visualisation responsive.
pub const HARD_GRAPH_NODE_CAP: usize = 2_000;

/// One registered brain's already-approved graph sources.
///
/// `allowlisted_markdown_paths` is an exact list of paths relative to
/// `root_path`, not a glob and not a directory to recurse through. The caller
/// owns the policy preflight which decides that a path is public enough for
/// graphing. `protected_relative_paths` contains literal relative path
/// prefixes which must be excluded even if mistakenly present in the allowlist.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphBrainSource {
    pub brain_id: String,
    pub brain_name: String,
    pub root_path: PathBuf,
    #[serde(default)]
    pub allowlisted_markdown_paths: Vec<PathBuf>,
    #[serde(default)]
    pub protected_relative_paths: Vec<PathBuf>,
}

impl GraphBrainSource {
    /// Construct an empty source which callers can populate after policy
    /// preflight. Keeping the allowlist empty is intentionally safe by default.
    pub fn new(
        brain_id: impl Into<String>,
        brain_name: impl Into<String>,
        root_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            brain_id: brain_id.into(),
            brain_name: brain_name.into(),
            root_path: root_path.into(),
            allowlisted_markdown_paths: Vec::new(),
            protected_relative_paths: Vec::new(),
        }
    }
}

/// Build-time limits for an atlas snapshot.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphBuildOptions {
    /// Total node count, including the Kairos and brain-cluster nodes.
    #[serde(default = "default_graph_node_cap")]
    pub node_cap: usize,
}

impl Default for GraphBuildOptions {
    fn default() -> Self {
        Self {
            node_cap: DEFAULT_GRAPH_NODE_CAP,
        }
    }
}

impl GraphBuildOptions {
    /// Clamp a user setting to the safe graph limit. A zero value still yields
    /// a single central Kairos node rather than an unusable empty graph.
    pub fn effective_node_cap(&self) -> usize {
        self.node_cap.clamp(1, HARD_GRAPH_NODE_CAP)
    }
}

fn default_graph_node_cap() -> usize {
    DEFAULT_GRAPH_NODE_CAP
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeKind {
    Kairos,
    Brain,
    Note,
    Bridge,
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeKind {
    /// Kairos is connected to each registered brain cluster.
    Owns,
    /// A brain cluster is connected to each included note.
    Contains,
    WikiLink,
    Embed,
    MarkdownLink,
    /// An explicit cross-brain bridge declared by safe frontmatter.
    CrossBrainBridge,
    Tag,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphClusterStatus {
    Online,
    Offline,
    Partial,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDiagnosticKind {
    /// A registered root cannot currently be reached.
    Offline,
    /// A previously indexed source has changed, disappeared, or is no longer
    /// allowlisted and should be rebuilt.
    Stale,
    /// The snapshot is intentionally incomplete, normally due to its cap.
    Partial,
    /// An unsafe or unsupported supplied path was ignored.
    Skipped,
}

/// A single metadata-only atlas node. It never contains note body text.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub kind: GraphNodeKind,
    pub label: String,
    pub brain_id: Option<String>,
    pub cluster_id: Option<String>,
    pub relative_path: Option<String>,
    pub modified_at: Option<DateTime<Utc>>,
}

/// A directed explicit relationship in the atlas.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: GraphEdgeKind,
}

/// A visual grouping for a brain. Its node is connected directly to Kairos.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphCluster {
    pub id: String,
    pub brain_id: String,
    pub label: String,
    pub status: GraphClusterStatus,
    pub note_count: usize,
}

/// A non-fatal scan or freshness warning suitable for the graph UI.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDiagnostic {
    pub kind: GraphDiagnosticKind,
    pub brain_id: Option<String>,
    pub relative_path: Option<String>,
    pub message: String,
}

/// A complete graph snapshot. The caller may cache this type without storing
/// any note content.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphIndex {
    pub generated_at: DateTime<Utc>,
    pub node_cap: usize,
    pub capped: bool,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub clusters: Vec<GraphCluster>,
    pub diagnostics: Vec<GraphDiagnostic>,
}

impl GraphIndex {
    /// Probe a cached index without reading note text. This reports roots that
    /// are offline and sources which have changed since `generated_at`.
    pub fn freshness(&self, sources: &[GraphBrainSource]) -> Vec<GraphDiagnostic> {
        graph_freshness(self, sources)
    }
}

/// A parsed explicit Markdown relationship. Targets are transient parser
/// output; only relationships that resolve to safe nodes enter [`GraphIndex`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplicitReference {
    pub target: String,
    pub kind: GraphEdgeKind,
}

/// Explicit links and tags extracted from one approved Markdown note.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExplicitReferences {
    pub links: Vec<ExplicitReference>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug)]
struct PreparedNote {
    relative_path: String,
    canonical_path: PathBuf,
    modified_at: Option<DateTime<Utc>>,
    node_id: Option<String>,
}

#[derive(Clone, Debug)]
struct PreparedBrain {
    brain_id: String,
    cluster_index: usize,
    brain_node_id: Option<String>,
    root: PathBuf,
    notes: Vec<PreparedNote>,
}

/// Build a whole-brain atlas from exact, previously approved note paths.
///
/// A bad path or temporarily unavailable root is represented as a diagnostic
/// instead of making every other brain unavailable. This function never
/// recursively scans a root, and it only reads a note after rejecting protected
/// paths and validating that the canonical file remains inside its root.
pub fn build_graph_index(sources: &[GraphBrainSource], options: GraphBuildOptions) -> GraphIndex {
    let node_cap = options.effective_node_cap();
    let mut index = GraphIndex {
        generated_at: Utc::now(),
        node_cap,
        capped: false,
        nodes: Vec::new(),
        edges: Vec::new(),
        clusters: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut edge_keys = HashSet::new();

    let _ = push_node(
        &mut index,
        GraphNode {
            id: KAIROS_NODE_ID.to_owned(),
            kind: GraphNodeKind::Kairos,
            label: "Kairos".to_owned(),
            brain_id: None,
            cluster_id: None,
            relative_path: None,
            modified_at: None,
        },
    );

    let mut ordered_sources = sources.to_vec();
    ordered_sources.sort_by(|left, right| {
        left.brain_id
            .cmp(&right.brain_id)
            .then_with(|| left.brain_name.cmp(&right.brain_name))
            .then_with(|| left.root_path.cmp(&right.root_path))
    });

    let mut known_brains = HashSet::new();
    let mut prepared_brains = Vec::new();
    for source in ordered_sources {
        let brain_id = source.brain_id.trim().to_owned();
        if brain_id.is_empty() {
            push_diagnostic(
                &mut index,
                GraphDiagnosticKind::Skipped,
                None,
                None,
                "A graph source without a brain ID was ignored.".to_owned(),
            );
            continue;
        }
        if !known_brains.insert(brain_id.clone()) {
            push_diagnostic(
                &mut index,
                GraphDiagnosticKind::Skipped,
                Some(brain_id),
                None,
                "A duplicate graph source for this brain was ignored.".to_owned(),
            );
            continue;
        }

        let cluster_id = brain_node_id(&brain_id);
        let cluster_index = index.clusters.len();
        index.clusters.push(GraphCluster {
            id: cluster_id.clone(),
            brain_id: brain_id.clone(),
            label: non_empty_label(&source.brain_name, &brain_id),
            status: GraphClusterStatus::Online,
            note_count: 0,
        });

        let root = match fs::canonicalize(&source.root_path) {
            Ok(root) if root.is_dir() => root,
            Ok(_) => {
                index.clusters[cluster_index].status = GraphClusterStatus::Offline;
                push_diagnostic(
                    &mut index,
                    GraphDiagnosticKind::Offline,
                    Some(brain_id),
                    None,
                    format!(
                        "Registered graph root is not a directory: {}.",
                        source.root_path.display()
                    ),
                );
                continue;
            }
            Err(error) => {
                index.clusters[cluster_index].status = GraphClusterStatus::Offline;
                push_diagnostic(
                    &mut index,
                    GraphDiagnosticKind::Offline,
                    Some(brain_id),
                    None,
                    format!(
                        "Registered graph root is unavailable: {} ({error}).",
                        source.root_path.display()
                    ),
                );
                continue;
            }
        };

        let cluster_label = index.clusters[cluster_index].label.clone();
        let brain_node_id = if push_node(
            &mut index,
            GraphNode {
                id: cluster_id.clone(),
                kind: GraphNodeKind::Brain,
                label: cluster_label,
                brain_id: Some(brain_id.clone()),
                cluster_id: Some(cluster_id.clone()),
                relative_path: None,
                modified_at: None,
            },
        ) {
            push_edge(
                &mut index.edges,
                &mut edge_keys,
                KAIROS_NODE_ID,
                &cluster_id,
                GraphEdgeKind::Owns,
            );
            Some(cluster_id)
        } else {
            index.clusters[cluster_index].status = GraphClusterStatus::Partial;
            mark_capped(&mut index, Some(brain_id.clone()));
            None
        };

        let protected_paths = normalized_protected_paths(&source.protected_relative_paths);
        let notes = prepare_notes(&source, &root, &protected_paths, &mut index, &brain_id);
        prepared_brains.push(PreparedBrain {
            brain_id,
            cluster_index,
            brain_node_id,
            root,
            notes,
        });
    }

    // Reserve the limited note slots before parsing bodies. This both keeps the
    // cap exact and avoids reading notes which cannot appear in the graph.
    for brain in &mut prepared_brains {
        for note in &mut brain.notes {
            if index.nodes.len() >= node_cap {
                index.clusters[brain.cluster_index].status = GraphClusterStatus::Partial;
                mark_capped(&mut index, Some(brain.brain_id.clone()));
                continue;
            }
            let node_id = note_node_id(&brain.brain_id, &note.relative_path);
            if push_node(
                &mut index,
                GraphNode {
                    id: node_id.clone(),
                    kind: GraphNodeKind::Note,
                    label: note_label(&note.relative_path),
                    brain_id: Some(brain.brain_id.clone()),
                    cluster_id: Some(brain_node_id(&brain.brain_id)),
                    relative_path: Some(note.relative_path.clone()),
                    modified_at: note.modified_at,
                },
            ) {
                if let Some(brain_node_id) = &brain.brain_node_id {
                    push_edge(
                        &mut index.edges,
                        &mut edge_keys,
                        brain_node_id,
                        &node_id,
                        GraphEdgeKind::Contains,
                    );
                }
                index.clusters[brain.cluster_index].note_count += 1;
                note.node_id = Some(node_id);
            }
        }
    }

    let lookup = build_note_lookup(&prepared_brains);
    let mut tag_nodes = HashMap::<String, String>::new();
    for brain in &prepared_brains {
        for note in &brain.notes {
            let Some(source_id) = note.node_id.as_deref() else {
                continue;
            };
            // Note text is intentionally scoped to this block. Nothing from it
            // is retained in the snapshot other than derived links and tags.
            let text = match fs::read_to_string(&note.canonical_path) {
                Ok(text) => text,
                Err(error) => {
                    push_diagnostic(
                        &mut index,
                        GraphDiagnosticKind::Stale,
                        Some(brain.brain_id.clone()),
                        Some(note.relative_path.clone()),
                        format!("Approved note could not be read while indexing ({error})."),
                    );
                    continue;
                }
            };
            let references = extract_explicit_references(&text);

            if let Some(target) = bridge_source_of_truth(&text) {
                if let Some(node) = index.nodes.iter_mut().find(|node| node.id == source_id) {
                    node.kind = GraphNodeKind::Bridge;
                }
                if let Ok(target) = fs::canonicalize(target) {
                    let target_brain = prepared_brains.iter().find(|candidate| {
                        target.starts_with(&candidate.root) && candidate.brain_id != brain.brain_id
                    });
                    if let Some(target_brain) = target_brain {
                        let exact_target = target_brain.notes.iter().find_map(|candidate| {
                            (candidate.canonical_path == target)
                                .then(|| candidate.node_id.clone())
                                .flatten()
                        });
                        let target_id = exact_target.or_else(|| target_brain.brain_node_id.clone());
                        if let Some(target_id) = target_id {
                            push_edge(
                                &mut index.edges,
                                &mut edge_keys,
                                source_id,
                                &target_id,
                                GraphEdgeKind::CrossBrainBridge,
                            );
                        }
                    }
                }
            }
            drop(text);

            for reference in references.links {
                let Some(target_id) = resolve_reference(
                    &brain.brain_id,
                    &note.relative_path,
                    &reference.target,
                    &reference.kind,
                    &lookup,
                ) else {
                    // Do not create unresolved nodes. They can represent a
                    // protected, unavailable, or external source and should
                    // not become graph metadata.
                    continue;
                };
                push_edge(
                    &mut index.edges,
                    &mut edge_keys,
                    source_id,
                    &target_id,
                    reference.kind,
                );
            }

            for tag in references.tags {
                let tag_node_id = if let Some(id) = tag_nodes.get(&tag) {
                    id.clone()
                } else {
                    let id = tag_node_id(&tag);
                    if !push_node(
                        &mut index,
                        GraphNode {
                            id: id.clone(),
                            kind: GraphNodeKind::Tag,
                            label: format!("#{tag}"),
                            brain_id: None,
                            cluster_id: None,
                            relative_path: None,
                            modified_at: None,
                        },
                    ) {
                        index.clusters[brain.cluster_index].status = GraphClusterStatus::Partial;
                        mark_capped(&mut index, Some(brain.brain_id.clone()));
                        continue;
                    }
                    tag_nodes.insert(tag, id.clone());
                    id
                };
                push_edge(
                    &mut index.edges,
                    &mut edge_keys,
                    source_id,
                    &tag_node_id,
                    GraphEdgeKind::Tag,
                );
            }
        }
    }

    index
}

/// Check whether a cached atlas is still representative without reading any
/// note body. Supplying the same policy-filtered source list is required: a
/// path removed from the allowlist is treated as stale until a fresh index is
/// built.
pub fn graph_freshness(index: &GraphIndex, sources: &[GraphBrainSource]) -> Vec<GraphDiagnostic> {
    let mut diagnostics = Vec::new();
    let mut sources_by_id = BTreeMap::new();
    for source in sources {
        if !source.brain_id.trim().is_empty() {
            sources_by_id
                .entry(source.brain_id.trim().to_owned())
                .or_insert(source);
        }
    }

    let mut indexed_note_paths = HashSet::new();
    for node in &index.nodes {
        if node.kind != GraphNodeKind::Note {
            continue;
        }
        let (Some(brain_id), Some(relative_path)) =
            (node.brain_id.as_deref(), node.relative_path.as_deref())
        else {
            continue;
        };
        indexed_note_paths.insert((brain_id.to_owned(), relative_path.to_owned()));
    }

    for (brain_id, source) in &sources_by_id {
        let root = match fs::canonicalize(&source.root_path) {
            Ok(root) if root.is_dir() => root,
            Ok(_) => {
                freshness_diagnostic(
                    &mut diagnostics,
                    GraphDiagnosticKind::Offline,
                    Some(brain_id.clone()),
                    None,
                    format!(
                        "Registered graph root is not a directory: {}.",
                        source.root_path.display()
                    ),
                );
                continue;
            }
            Err(error) => {
                freshness_diagnostic(
                    &mut diagnostics,
                    GraphDiagnosticKind::Offline,
                    Some(brain_id.clone()),
                    None,
                    format!(
                        "Registered graph root is unavailable: {} ({error}).",
                        source.root_path.display()
                    ),
                );
                continue;
            }
        };

        let protected_paths = normalized_protected_paths(&source.protected_relative_paths);
        for relative_path in normalized_allowlisted_paths(&source.allowlisted_markdown_paths) {
            if is_protected_relative_path(&relative_path, &protected_paths) {
                continue;
            }
            let key = (brain_id.clone(), relative_path.clone());
            if !indexed_note_paths.contains(&key) {
                // New safe paths do not mean an existing snapshot is unsafe;
                // flag it so the UI can offer a rebuild.
                freshness_diagnostic(
                    &mut diagnostics,
                    GraphDiagnosticKind::Stale,
                    Some(brain_id.clone()),
                    Some(relative_path),
                    "Approved graph source is not present in the cached index.".to_owned(),
                );
                continue;
            }

            let candidate = root.join(&relative_path);
            let metadata = match fs::symlink_metadata(&candidate) {
                Ok(metadata)
                    if metadata.file_type().is_file() && !metadata.file_type().is_symlink() =>
                {
                    metadata
                }
                Ok(_) => {
                    freshness_diagnostic(
                        &mut diagnostics,
                        GraphDiagnosticKind::Stale,
                        Some(brain_id.clone()),
                        Some(relative_path),
                        "Approved graph source is no longer a regular Markdown file.".to_owned(),
                    );
                    continue;
                }
                Err(error) => {
                    freshness_diagnostic(
                        &mut diagnostics,
                        GraphDiagnosticKind::Stale,
                        Some(brain_id.clone()),
                        Some(relative_path),
                        format!("Approved graph source is unavailable ({error})."),
                    );
                    continue;
                }
            };
            let canonical = match fs::canonicalize(&candidate) {
                Ok(canonical) if canonical.starts_with(&root) => canonical,
                Ok(_) => {
                    freshness_diagnostic(
                        &mut diagnostics,
                        GraphDiagnosticKind::Stale,
                        Some(brain_id.clone()),
                        Some(relative_path),
                        "Approved graph source now resolves outside its registered root."
                            .to_owned(),
                    );
                    continue;
                }
                Err(error) => {
                    freshness_diagnostic(
                        &mut diagnostics,
                        GraphDiagnosticKind::Stale,
                        Some(brain_id.clone()),
                        Some(relative_path),
                        format!("Approved graph source cannot be resolved ({error})."),
                    );
                    continue;
                }
            };
            let _ = canonical;
            if metadata
                .modified()
                .ok()
                .map(DateTime::<Utc>::from)
                .is_some_and(|modified_at| modified_at > index.generated_at)
            {
                freshness_diagnostic(
                    &mut diagnostics,
                    GraphDiagnosticKind::Stale,
                    Some(brain_id.clone()),
                    Some(relative_path),
                    "Approved graph source changed after this index was built.".to_owned(),
                );
            }
        }
    }

    for (brain_id, relative_path) in indexed_note_paths {
        let Some(source) = sources_by_id.get(&brain_id) else {
            freshness_diagnostic(
                &mut diagnostics,
                GraphDiagnosticKind::Stale,
                Some(brain_id),
                Some(relative_path),
                "Indexed brain is no longer registered as a graph source.".to_owned(),
            );
            continue;
        };
        let protected_paths = normalized_protected_paths(&source.protected_relative_paths);
        let allowlisted = normalized_allowlisted_paths(&source.allowlisted_markdown_paths);
        if is_protected_relative_path(&relative_path, &protected_paths)
            || !allowlisted.contains(&relative_path)
        {
            freshness_diagnostic(
                &mut diagnostics,
                GraphDiagnosticKind::Stale,
                Some(brain_id),
                Some(relative_path),
                "Indexed note is no longer approved for graphing; rebuild required.".to_owned(),
            );
        }
    }

    diagnostics
}

/// Extract explicit Obsidian-style links, embeds, Markdown links, and inline
/// tags. Frontmatter and fenced/inline code are excluded from parsing.
pub fn extract_explicit_references(markdown: &str) -> ExplicitReferences {
    let visible = markdown_without_metadata_or_code(markdown);
    let mut links = Vec::new();
    let mut seen_links = HashSet::new();
    let mut index = 0;
    let bytes = visible.as_bytes();

    while index < bytes.len() {
        if visible[index..].starts_with("![[") || visible[index..].starts_with("[[") {
            let is_embed = visible[index..].starts_with("![[");
            let content_start = index + if is_embed { 3 } else { 2 };
            if let Some(relative_end) = visible[content_start..].find("]]") {
                let end = content_start + relative_end;
                if let Some(target) = normalize_wikilink_target(&visible[content_start..end]) {
                    let kind = if is_embed {
                        GraphEdgeKind::Embed
                    } else {
                        GraphEdgeKind::WikiLink
                    };
                    push_reference(&mut links, &mut seen_links, target, kind);
                }
                index = end + 2;
                continue;
            }
        }

        if bytes[index] == b'[' && !visible[index..].starts_with("[[") {
            let is_embed = index > 0 && bytes[index - 1] == b'!';
            if let Some(label_end_offset) = visible[index + 1..].find(']') {
                let label_end = index + 1 + label_end_offset;
                if !visible[label_end..].starts_with("](") {
                    index += char_width(bytes[index]);
                    continue;
                }
                let target_start = label_end + 2;
                if let Some(target_end_offset) = visible[target_start..].find(')') {
                    let target_end = target_start + target_end_offset;
                    if let Some(target) =
                        normalize_markdown_target(&visible[target_start..target_end])
                    {
                        let kind = if is_embed {
                            GraphEdgeKind::Embed
                        } else {
                            GraphEdgeKind::MarkdownLink
                        };
                        push_reference(&mut links, &mut seen_links, target, kind);
                    }
                    index = target_end + 1;
                    continue;
                }
            }
        }
        index += char_width(bytes[index]);
    }

    ExplicitReferences {
        links,
        tags: extract_tags(&mask_link_spans(&visible)),
    }
}

fn prepare_notes(
    source: &GraphBrainSource,
    root: &Path,
    protected_paths: &[String],
    index: &mut GraphIndex,
    brain_id: &str,
) -> Vec<PreparedNote> {
    let mut notes = Vec::new();
    for relative_path in normalized_allowlisted_paths(&source.allowlisted_markdown_paths) {
        if is_protected_relative_path(&relative_path, protected_paths) {
            // This path is rejected before resolution or a filesystem read.
            continue;
        }
        let candidate = root.join(&relative_path);
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) => {
                push_diagnostic(
                    index,
                    GraphDiagnosticKind::Stale,
                    Some(brain_id.to_owned()),
                    Some(relative_path),
                    format!("Approved graph source is unavailable ({error})."),
                );
                continue;
            }
        };
        if metadata.file_type().is_symlink() {
            push_diagnostic(
                index,
                GraphDiagnosticKind::Skipped,
                Some(brain_id.to_owned()),
                Some(relative_path),
                "Symbolic links are not graph sources.".to_owned(),
            );
            continue;
        }
        if !metadata.file_type().is_file() {
            push_diagnostic(
                index,
                GraphDiagnosticKind::Skipped,
                Some(brain_id.to_owned()),
                Some(relative_path),
                "Only regular Markdown files are graph sources.".to_owned(),
            );
            continue;
        }
        let canonical_path = match fs::canonicalize(&candidate) {
            Ok(path) if path.starts_with(root) => path,
            Ok(_) => {
                push_diagnostic(
                    index,
                    GraphDiagnosticKind::Skipped,
                    Some(brain_id.to_owned()),
                    Some(relative_path),
                    "Graph source resolves outside its registered root.".to_owned(),
                );
                continue;
            }
            Err(error) => {
                push_diagnostic(
                    index,
                    GraphDiagnosticKind::Stale,
                    Some(brain_id.to_owned()),
                    Some(relative_path),
                    format!("Graph source cannot be resolved ({error})."),
                );
                continue;
            }
        };
        notes.push(PreparedNote {
            relative_path,
            canonical_path,
            modified_at: metadata.modified().ok().map(DateTime::<Utc>::from),
            node_id: None,
        });
    }
    notes
}

fn normalized_allowlisted_paths(paths: &[PathBuf]) -> BTreeSet<String> {
    paths
        .iter()
        .filter_map(|path| normalized_markdown_relative_path(path))
        .collect()
}

fn normalized_protected_paths(paths: &[PathBuf]) -> Vec<String> {
    let mut normalized = paths
        .iter()
        .filter_map(|path| normalized_relative_path(path))
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn normalized_markdown_relative_path(path: &Path) -> Option<String> {
    let relative = normalized_relative_path(path)?;
    Path::new(&relative)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        .then_some(relative)
}

fn normalized_relative_path(path: &Path) -> Option<String> {
    if path.is_absolute() || path.as_os_str().is_empty() {
        return None;
    }
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(component) => {
                let component = component.to_str()?;
                if component.is_empty() {
                    return None;
                }
                components.push(component);
            }
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!components.is_empty()).then(|| components.join("/"))
}

fn is_protected_relative_path(relative_path: &str, protected_paths: &[String]) -> bool {
    let components = relative_path.split('/').collect::<Vec<_>>();
    if components.iter().any(|component| {
        *component == "90_Private"
            || component.starts_with('.')
            || component.eq_ignore_ascii_case("private")
    }) {
        return true;
    }
    protected_paths.iter().any(|prefix| {
        relative_path == prefix
            || (relative_path.starts_with(prefix)
                && relative_path
                    .as_bytes()
                    .get(prefix.len())
                    .is_some_and(|character| *character == b'/'))
    })
}

fn build_note_lookup(brains: &[PreparedBrain]) -> HashMap<(String, String), Vec<String>> {
    let mut lookup = HashMap::new();
    for brain in brains {
        for note in &brain.notes {
            let Some(node_id) = &note.node_id else {
                continue;
            };
            let Some(path_key) = reference_key(&note.relative_path) else {
                continue;
            };
            lookup
                .entry((brain.brain_id.clone(), path_key.clone()))
                .or_insert_with(Vec::new)
                .push(node_id.clone());
            if let Some(file_name) = path_key.rsplit('/').next()
                && file_name != path_key
            {
                lookup
                    .entry((brain.brain_id.clone(), file_name.to_owned()))
                    .or_insert_with(Vec::new)
                    .push(node_id.clone());
            }
        }
    }
    lookup
}

fn resolve_reference(
    brain_id: &str,
    source_relative_path: &str,
    target: &str,
    kind: &GraphEdgeKind,
    lookup: &HashMap<(String, String), Vec<String>>,
) -> Option<String> {
    let target_key = reference_key(target)?;
    let mut candidates = Vec::new();

    if matches!(kind, GraphEdgeKind::MarkdownLink | GraphEdgeKind::Embed)
        && let Some(parent) = Path::new(source_relative_path).parent()
        && !parent.as_os_str().is_empty()
    {
        let relative_target = parent.join(&target_key);
        if let Some(key) = reference_key(&relative_target.to_string_lossy()) {
            candidates.push(key);
        }
    }
    candidates.push(target_key);

    for key in candidates {
        let Some(matches) = lookup.get(&(brain_id.to_owned(), key)) else {
            continue;
        };
        if matches.len() == 1 {
            return matches.first().cloned();
        }
    }
    None
}

fn reference_key(reference: &str) -> Option<String> {
    let reference = reference.trim().trim_matches('"').trim_matches('\'');
    let reference = reference
        .split('|')
        .next()
        .unwrap_or_default()
        .split('#')
        .next()
        .unwrap_or_default()
        .split('^')
        .next()
        .unwrap_or_default()
        .trim()
        .replace('\\', "/");
    if reference.is_empty()
        || reference.contains("://")
        || reference.contains(':')
        || reference.starts_with('#')
    {
        return None;
    }
    let path = Path::new(&reference);
    let normalized = normalized_relative_path(path)?;
    let normalized = normalized
        .strip_suffix(".md")
        .or_else(|| normalized.strip_suffix(".MD"))
        .unwrap_or(&normalized)
        .to_owned();
    (!normalized.is_empty()).then_some(normalized)
}

fn normalize_wikilink_target(raw_target: &str) -> Option<String> {
    reference_key(raw_target)
}

fn normalize_markdown_target(raw_target: &str) -> Option<String> {
    let target = raw_target.trim();
    let target = target
        .strip_prefix('<')
        .and_then(|target| target.strip_suffix('>'))
        .unwrap_or(target);
    let target = target.split_whitespace().next().unwrap_or_default();
    reference_key(target)
}

fn markdown_without_metadata_or_code(markdown: &str) -> String {
    let mut output = String::new();
    let mut first_line = true;
    let mut in_frontmatter = false;
    let mut in_fence: Option<&str> = None;

    for line in markdown.lines() {
        if first_line {
            first_line = false;
            if line.trim() == "---" {
                in_frontmatter = true;
                continue;
            }
        }
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
            }
            continue;
        }

        let trimmed = line.trim_start();
        if let Some(fence) = in_fence {
            if trimmed.starts_with(fence) {
                in_fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            in_fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            in_fence = Some("~~~");
            continue;
        }
        output.push_str(&strip_inline_code(line));
        output.push('\n');
    }
    output
}

fn strip_inline_code(line: &str) -> String {
    let mut output = String::new();
    let mut in_code = false;
    let mut escaped = false;
    for character in line.chars() {
        if character == '`' && !escaped {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            output.push(character);
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    output
}

fn extract_tags(markdown: &str) -> Vec<String> {
    let mut tags = BTreeSet::new();
    let characters = markdown.char_indices().collect::<Vec<_>>();
    for (index, (offset, character)) in characters.iter().enumerate() {
        if *character != '#' {
            continue;
        }
        let previous = index
            .checked_sub(1)
            .and_then(|previous| characters.get(previous))
            .map(|(_, character)| *character);
        if previous.is_some_and(is_tag_character) {
            continue;
        }
        let start = *offset + character.len_utf8();
        let mut end = start;
        for character in markdown[start..].chars() {
            if is_tag_character(character) {
                end += character.len_utf8();
            } else {
                break;
            }
        }
        if end == start {
            continue;
        }
        let tag = markdown[start..end].trim_matches('/').to_lowercase();
        if !tag.is_empty()
            && tag != "private"
            && !tag
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
            && !is_inline_style_hex_colour(markdown, *offset, &tag)
        {
            tags.insert(tag);
        }
    }
    tags.into_iter().collect()
}

/// Replace Markdown link syntax with whitespace before tag scanning. A heading
/// fragment such as `[[#Figure Index]]` is a link target, never a `#figure`
/// tag node. Keeping newlines preserves the surrounding parser boundaries.
fn mask_link_spans(markdown: &str) -> String {
    let bytes = markdown.as_bytes();
    let mut output = markdown.as_bytes().to_vec();
    let mut index = 0;
    while index < bytes.len() {
        let wiki_start =
            markdown[index..].starts_with("[[") || markdown[index..].starts_with("![[");
        if wiki_start {
            let start = index;
            let content_start = index
                + if markdown[index..].starts_with("![[") {
                    3
                } else {
                    2
                };
            if let Some(relative_end) = markdown[content_start..].find("]]") {
                let end = content_start + relative_end + 2;
                for byte in &mut output[start..end] {
                    if *byte != b'\n' {
                        *byte = b' ';
                    }
                }
                index = end;
                continue;
            }
        }
        if bytes[index] == b'[' && !markdown[index..].starts_with("[[") {
            if let Some(label_end_offset) = markdown[index + 1..].find("](") {
                let target_start = index + 1 + label_end_offset + 2;
                if let Some(target_end_offset) = markdown[target_start..].find(')') {
                    let end = target_start + target_end_offset + 1;
                    for byte in &mut output[index..end] {
                        if *byte != b'\n' {
                            *byte = b' ';
                        }
                    }
                    index = end;
                    continue;
                }
            }
        }
        index += char_width(bytes[index]);
    }
    String::from_utf8(output).unwrap_or_else(|_| markdown.to_owned())
}

fn bridge_source_of_truth(markdown: &str) -> Option<PathBuf> {
    let remaining = markdown.strip_prefix("---\n")?;
    let end = remaining.find("\n---")?;
    let mut is_bridge = false;
    let mut source = None;
    for line in remaining[..end].lines() {
        if line.trim() == "type: bridge" {
            is_bridge = true;
        }
        if let Some(value) = line.strip_prefix("source_of_truth:") {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                source = Some(PathBuf::from(value));
            }
        }
    }
    is_bridge
        .then_some(source?)
        .filter(|path| path.is_absolute())
}

/// CSS colour values look exactly like compact Obsidian tags (for example,
/// `background:#22C55E`). Only ignore them when they occur in an inline HTML
/// `style` attribute: a standalone `#c0ffee` is still a valid tag.
fn is_inline_style_hex_colour(markdown: &str, hash_offset: usize, tag: &str) -> bool {
    if !matches!(tag.len(), 3 | 4 | 6 | 8)
        || !tag.bytes().all(|character| character.is_ascii_hexdigit())
    {
        return false;
    }

    let before_hash = &markdown[..hash_offset];
    if before_hash
        .chars()
        .rev()
        .find(|character| !character.is_whitespace())
        != Some(':')
    {
        return false;
    }

    let Some(open_tag_offset) = before_hash.rfind('<') else {
        return false;
    };
    let open_tag = &before_hash[open_tag_offset..];
    !open_tag.contains('>') && html_open_tag_has_style_attribute(open_tag)
}

fn html_open_tag_has_style_attribute(open_tag: &str) -> bool {
    let bytes = open_tag.as_bytes();
    let mut index = 0;
    while index + b"style".len() <= bytes.len() {
        if bytes[index..index + b"style".len()].eq_ignore_ascii_case(b"style") {
            let has_name_boundary =
                index == 0 || !is_html_attribute_name_character(bytes[index - 1]);
            let mut next = index + b"style".len();
            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                next += 1;
            }
            if has_name_boundary && bytes.get(next) == Some(&b'=') {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn is_html_attribute_name_character(character: u8) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, b'-' | b'_' | b':')
}

fn is_tag_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '-' | '/')
}

fn push_reference(
    links: &mut Vec<ExplicitReference>,
    seen_links: &mut HashSet<(String, GraphEdgeKind)>,
    target: String,
    kind: GraphEdgeKind,
) {
    if seen_links.insert((target.clone(), kind.clone())) {
        links.push(ExplicitReference { target, kind });
    }
}

fn char_width(byte: u8) -> usize {
    if byte < 0x80 {
        1
    } else if byte & 0b1110_0000 == 0b1100_0000 {
        2
    } else if byte & 0b1111_0000 == 0b1110_0000 {
        3
    } else {
        4
    }
}

fn push_node(index: &mut GraphIndex, node: GraphNode) -> bool {
    if index.nodes.len() >= index.node_cap {
        return false;
    }
    index.nodes.push(node);
    true
}

fn push_edge(
    edges: &mut Vec<GraphEdge>,
    edge_keys: &mut HashSet<(String, String, GraphEdgeKind)>,
    source: &str,
    target: &str,
    kind: GraphEdgeKind,
) {
    let key = (source.to_owned(), target.to_owned(), kind.clone());
    if !edge_keys.insert(key) {
        return;
    }
    edges.push(GraphEdge {
        id: format!("{}:{}>{}", edge_kind_key(&kind), source, target),
        source: source.to_owned(),
        target: target.to_owned(),
        kind,
    });
}

fn edge_kind_key(kind: &GraphEdgeKind) -> &'static str {
    match kind {
        GraphEdgeKind::Owns => "owns",
        GraphEdgeKind::Contains => "contains",
        GraphEdgeKind::WikiLink => "wiki",
        GraphEdgeKind::Embed => "embed",
        GraphEdgeKind::MarkdownLink => "markdown",
        GraphEdgeKind::CrossBrainBridge => "bridge",
        GraphEdgeKind::Tag => "tag",
    }
}

fn push_diagnostic(
    index: &mut GraphIndex,
    kind: GraphDiagnosticKind,
    brain_id: Option<String>,
    relative_path: Option<String>,
    message: String,
) {
    index.diagnostics.push(GraphDiagnostic {
        kind,
        brain_id,
        relative_path,
        message,
    });
}

fn freshness_diagnostic(
    diagnostics: &mut Vec<GraphDiagnostic>,
    kind: GraphDiagnosticKind,
    brain_id: Option<String>,
    relative_path: Option<String>,
    message: String,
) {
    let duplicate = diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == kind
            && diagnostic.brain_id == brain_id
            && diagnostic.relative_path == relative_path
    });
    if !duplicate {
        diagnostics.push(GraphDiagnostic {
            kind,
            brain_id,
            relative_path,
            message,
        });
    }
}

fn mark_capped(index: &mut GraphIndex, brain_id: Option<String>) {
    if !index.capped {
        index.capped = true;
        push_diagnostic(
            index,
            GraphDiagnosticKind::Partial,
            brain_id,
            None,
            format!(
                "Graph node cap of {} reached; rebuild with a larger cap to include more metadata.",
                index.node_cap
            ),
        );
    }
}

fn non_empty_label(label: &str, fallback: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        fallback.to_owned()
    } else {
        label.to_owned()
    }
}

fn brain_node_id(brain_id: &str) -> String {
    format!("brain:{brain_id}")
}

fn note_node_id(brain_id: &str, relative_path: &str) -> String {
    format!("note:{brain_id}:{relative_path}")
}

fn tag_node_id(tag: &str) -> String {
    format!("tag:{tag}")
}

fn note_label(relative_path: &str) -> String {
    Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or(relative_path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(label: &str) -> Self {
            let unique = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "kairos-graph-{label}-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.path.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn source(temp: &TempDir, paths: &[&str]) -> GraphBrainSource {
        GraphBrainSource {
            brain_id: "everyday".to_owned(),
            brain_name: "Everyday Brain".to_owned(),
            root_path: temp.path.clone(),
            allowlisted_markdown_paths: paths.iter().map(PathBuf::from).collect(),
            protected_relative_paths: vec![PathBuf::from("90_Private")],
        }
    }

    #[test]
    fn builds_explicit_metadata_graph_without_private_nodes() {
        let temp = TempDir::new("explicit");
        temp.write(
            "Notes/A.md",
            "# A\n[[Notes/B|Bee]]\n![[Notes/B]]\n[Read B](B.md)\n#project/alpha #focus\n",
        );
        temp.write("Notes/B.md", "# B\n");
        // It is deliberately included in the supplied allowlist. The scanner
        // must still reject it before resolving or reading it.
        temp.write("90_Private/secret.md", "[[Notes/A]] #private");

        let index = build_graph_index(
            &[source(
                &temp,
                &["Notes/A.md", "Notes/B.md", "90_Private/secret.md"],
            )],
            GraphBuildOptions::default(),
        );

        assert!(index.nodes.iter().any(|node| node.id == KAIROS_NODE_ID));
        assert!(index.nodes.iter().any(|node| node.id == "brain:everyday"));
        assert!(
            index
                .nodes
                .iter()
                .any(|node| node.id == "note:everyday:Notes/A.md")
        );
        assert!(
            !index
                .nodes
                .iter()
                .any(|node| node.relative_path.as_deref() == Some("90_Private/secret.md"))
        );
        assert!(
            index
                .edges
                .iter()
                .any(|edge| edge.kind == GraphEdgeKind::WikiLink)
        );
        assert!(
            index
                .edges
                .iter()
                .any(|edge| edge.kind == GraphEdgeKind::Embed)
        );
        assert!(
            index
                .edges
                .iter()
                .any(|edge| edge.kind == GraphEdgeKind::MarkdownLink)
        );
        assert!(
            index
                .nodes
                .iter()
                .any(|node| node.id == "tag:project/alpha")
        );
        assert!(index.nodes.iter().all(|node| node.label != "secret"));
    }

    #[test]
    fn graph_does_not_create_nodes_for_css_colours_or_digit_started_tags() {
        let temp = TempDir::new("tag-noise");
        temp.write(
            "Map.md",
            "<span style=\"background:#22C55E;border-color:#0f08\">Green</span> #1 #project/alpha #c0ffee",
        );

        let index = build_graph_index(&[source(&temp, &["Map.md"])], GraphBuildOptions::default());

        assert!(
            index
                .nodes
                .iter()
                .any(|node| node.id == "tag:project/alpha")
        );
        assert!(index.nodes.iter().any(|node| node.id == "tag:c0ffee"));
        assert!(!index.nodes.iter().any(|node| node.id == "tag:22c55e"));
        assert!(!index.nodes.iter().any(|node| node.id == "tag:0f08"));
        assert!(!index.nodes.iter().any(|node| node.id == "tag:1"));
    }

    #[test]
    fn cap_is_total_and_never_exceeds_the_hard_limit() {
        let temp = TempDir::new("cap");
        temp.write("A.md", "#one");
        temp.write("B.md", "#two");
        temp.write("C.md", "#three");

        let index = build_graph_index(
            &[source(&temp, &["A.md", "B.md", "C.md"])],
            GraphBuildOptions { node_cap: 4 },
        );
        assert_eq!(index.node_cap, 4);
        assert!(index.nodes.len() <= 4);
        assert!(index.capped);
        assert!(
            index
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == GraphDiagnosticKind::Partial)
        );

        let hard_capped = build_graph_index(
            &[source(&temp, &["A.md"])],
            GraphBuildOptions { node_cap: 99_999 },
        );
        assert_eq!(hard_capped.node_cap, HARD_GRAPH_NODE_CAP);
    }

    #[test]
    fn reports_offline_and_stale_sources_without_reading_note_bodies() {
        let temp = TempDir::new("freshness");
        temp.write("Note.md", "# ordinary note");
        let source = source(&temp, &["Note.md"]);
        let index = build_graph_index(&[source.clone()], GraphBuildOptions::default());
        fs::remove_file(temp.path.join("Note.md")).unwrap();

        let diagnostics = graph_freshness(&index, &[source]);
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == GraphDiagnosticKind::Stale
                && diagnostic.relative_path.as_deref() == Some("Note.md")
        }));

        let missing = GraphBrainSource::new("offline", "Offline", temp.path.join("missing"));
        let offline = build_graph_index(&[missing], GraphBuildOptions::default());
        assert!(
            offline
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.kind == GraphDiagnosticKind::Offline)
        );
    }

    #[test]
    fn parser_ignores_frontmatter_and_code_but_keeps_explicit_links_and_tags() {
        let parsed = extract_explicit_references(
            "---\ntags: [hidden]\n---\n[[Real]] ![[Attachment]] [Doc](docs/Doc.md) #visible\n`[[Code]] #code`\n```md\n[[Fence]] #fence\n```\n# Heading\n",
        );
        assert_eq!(parsed.links.len(), 3);
        assert!(
            parsed
                .links
                .iter()
                .any(|link| link.target == "Real" && link.kind == GraphEdgeKind::WikiLink)
        );
        assert!(
            parsed
                .links
                .iter()
                .any(|link| link.target == "Attachment" && link.kind == GraphEdgeKind::Embed)
        );
        assert!(parsed.tags.contains(&"visible".to_owned()));
        assert!(!parsed.tags.contains(&"hidden".to_owned()));
        assert!(!parsed.tags.contains(&"code".to_owned()));
        assert!(!parsed.tags.contains(&"heading".to_owned()));
    }

    #[test]
    fn parser_excludes_hex_colours_in_html_style_attributes_but_keeps_real_tags() {
        let parsed = extract_explicit_references(
            "#project/alpha #2026 #1\n<span style=\"background:#22C55E;border-color: #0f08\">Green</span>\n<span>#c0ffee</span>\n",
        );

        assert!(parsed.tags.contains(&"project/alpha".to_owned()));
        assert!(parsed.tags.contains(&"c0ffee".to_owned()));
        assert!(!parsed.tags.contains(&"2026".to_owned()));
        assert!(!parsed.tags.contains(&"1".to_owned()));
        assert!(!parsed.tags.contains(&"22c55e".to_owned()));
        assert!(!parsed.tags.contains(&"0f08".to_owned()));
    }

    #[test]
    fn parser_does_not_turn_wikilink_heading_fragments_into_tags() {
        let parsed = extract_explicit_references("See [[#Figure Index]] and #real-tag.");
        assert!(parsed.links.is_empty());
        assert!(parsed.tags.contains(&"real-tag".to_owned()));
        assert!(!parsed.tags.contains(&"figure".to_owned()));
    }

    #[test]
    fn bridge_metadata_connects_registered_brains_without_exposing_a_path() {
        let everyday = TempDir::new("bridge-everyday");
        let phd = TempDir::new("bridge-phd");
        everyday.write(
            "06_Bridges/PhD Bridge.md",
            &format!(
                "---\ntype: bridge\nsource_of_truth: {}\n---\n# PhD bridge",
                phd.path.join("00_System/Current Context.md").display()
            ),
        );
        phd.write("00_System/Current Context.md", "# PhD context");
        let sources = vec![
            source(&everyday, &["06_Bridges/PhD Bridge.md"]),
            GraphBrainSource {
                brain_id: "phd".to_owned(),
                brain_name: "PhD Brain".to_owned(),
                root_path: phd.path.clone(),
                allowlisted_markdown_paths: vec![PathBuf::from("00_System/Current Context.md")],
                protected_relative_paths: Vec::new(),
            },
        ];
        let index = build_graph_index(&sources, GraphBuildOptions::default());
        assert!(index.nodes.iter().any(|node| {
            node.id == "note:everyday:06_Bridges/PhD Bridge.md"
                && node.kind == GraphNodeKind::Bridge
        }));
        assert!(index.edges.iter().any(|edge| {
            edge.kind == GraphEdgeKind::CrossBrainBridge
                && edge.source == "note:everyday:06_Bridges/PhD Bridge.md"
                && edge.target == "note:phd:00_System/Current Context.md"
        }));
        assert!(
            index
                .nodes
                .iter()
                .all(|node| !node.label.contains(&phd.path.display().to_string()))
        );
    }
}
