use crate::t_common;
use crate::t_sqlite::{Face, Person};
use instant_distance::{Builder as HnswBuilder, Search as HnswSearch};
// HnswMap stores face indices so PointId reshuffling does not break mapping.
use rand::seq::SliceRandom;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Clustering progress information
#[derive(Clone, serde::Serialize)]
pub struct ClusterProgress {
    pub phase: String, // "graph", "iterate", "converged", "assign", "thumbnail"
    pub current: usize,
    pub total: usize,
}

/// Graph build mode for face clustering (settings: `face.clusterMode`).
///
/// - **Auto** — exact for `n < CLUSTER_N_EXACT`, HNSW ANN at/above (default; blocked fallback).
/// - **Exact** — always row-wise all-pairs (power user / small libraries).
/// - **Fast** — always HNSW ANN (blocked fallback on build/search failure).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClusterMode {
    #[default]
    Auto,
    Exact,
    Fast,
}

impl ClusterMode {
    /// Parse UI/IPC string; unknown or empty → Auto.
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(|x| x.trim().to_ascii_lowercase()).as_deref() {
            Some("exact") => Self::Exact,
            Some("fast") => Self::Fast,
            Some("auto") | Some("") | None => Self::Auto,
            _ => Self::Auto,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Exact => "exact",
            Self::Fast => "fast",
        }
    }
}

/// Which concrete graph builder `mode` prefers for face count `n` (before fallback).
fn graph_strategy_for_mode(n: usize, mode: ClusterMode) -> &'static str {
    match mode {
        ClusterMode::Exact => "exact",
        ClusterMode::Fast => "ann",
        ClusterMode::Auto => {
            if n < t_common::CLUSTER_N_EXACT {
                "exact"
            } else {
                "ann"
            }
        }
    }
}

/// L2-normalized embedding point for HNSW (cosine distance = 1 - dot).
#[derive(Clone)]
struct FacePoint(Arc<[f32]>);

impl instant_distance::Point for FacePoint {
    fn distance(&self, other: &Self) -> f32 {
        cosine_distance(&self.0, &other.0)
    }
}

/// Consider pair (i, j) with i < j: same-file ban + threshold + dual Top-K insert.
#[inline]
fn try_add_edge(
    i: usize,
    j: usize,
    file_ids: &[i64],
    emb_i: &[f32],
    emb_j: &[f32],
    threshold: f32,
    k_neighbors: usize,
    candidate_lists: &mut [Vec<(usize, f32)>],
) {
    // Faces in the same file cannot be edges (prevents merging distinct people in same photo)
    if file_ids[i] == file_ids[j] {
        return;
    }
    let dist = cosine_distance(emb_i, emb_j);
    if dist < threshold {
        let weight = 1.0 - dist;
        // Square weight to punish weak links further
        let adjusted_weight = weight * weight;
        insert_top_k(&mut candidate_lists[i], (j, adjusted_weight), k_neighbors);
        insert_top_k(&mut candidate_lists[j], (i, adjusted_weight), k_neighbors);
    }
}

/// Build undirected Top-K similarity graph via exact all-pairs cosine (small-n path).
///
/// Edge weight = `(1 - cosine_distance)^2` when distance < threshold.
/// Same-file faces never form an edge. Cancel returns `Err("cancelled")`.
fn build_knn_graph_exact<P, C>(
    file_ids: &[i64],
    embeddings: &[Option<Vec<f32>>],
    threshold: f32,
    k_neighbors: usize,
    mut progress_fn: P,
    is_cancelled_fn: C,
) -> Result<Vec<Vec<(usize, f32)>>, String>
where
    P: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    let n = file_ids.len();
    debug_assert_eq!(n, embeddings.len());

    let mut candidate_lists: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    let total_pairs = n.saturating_mul(n.saturating_sub(1)) / 2;
    let mut pairs_done: usize = 0;
    let mut last_pct = 0;

    for i in 0..n {
        if is_cancelled_fn() {
            return Err("cancelled".to_string());
        }

        if let Some(emb_i) = &embeddings[i] {
            for j in (i + 1)..n {
                if let Some(emb_j) = &embeddings[j] {
                    try_add_edge(
                        i,
                        j,
                        file_ids,
                        emb_i,
                        emb_j,
                        threshold,
                        k_neighbors,
                        &mut candidate_lists,
                    );
                }
                pairs_done += 1;
            }
        } else {
            pairs_done += n.saturating_sub(i + 1);
        }

        // Report progress every 5%
        let current_pct = if total_pairs > 0 {
            pairs_done * 100 / total_pairs
        } else {
            100
        };
        if current_pct >= last_pct + 5 || pairs_done == total_pairs {
            progress_fn(ClusterProgress {
                phase: "graph".to_string(),
                current: current_pct,
                total: 100,
            });
            last_pct = current_pct;
        }
    }

    Ok(candidate_lists)
}

/// Exact cosine Top-K graph via block tiles (same edge semantics as all-pairs).
///
/// Still O(n²) flops, but better cache locality, progress by block-pair, and cancel between tiles.
/// Used when `n >= CLUSTER_N_EXACT` until ANN lands. See face-cluster-ann-plan Option B.
fn build_knn_graph_blocked<P, C>(
    file_ids: &[i64],
    embeddings: &[Option<Vec<f32>>],
    threshold: f32,
    k_neighbors: usize,
    block_size: usize,
    mut progress_fn: P,
    is_cancelled_fn: C,
) -> Result<Vec<Vec<(usize, f32)>>, String>
where
    P: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    let n = file_ids.len();
    debug_assert_eq!(n, embeddings.len());
    let block_size = block_size.max(1);
    let num_blocks = n.div_ceil(block_size);
    // Upper-triangle of block pairs (including diagonal).
    let total_block_pairs = num_blocks.saturating_mul(num_blocks.saturating_add(1)) / 2;
    let mut block_pairs_done = 0usize;
    let mut last_pct = 0usize;
    let mut candidate_lists: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];

    for bi in 0..num_blocks {
        for bj in bi..num_blocks {
            if is_cancelled_fn() {
                return Err("cancelled".to_string());
            }

            let i0 = bi * block_size;
            let i1 = ((bi + 1) * block_size).min(n);
            let j0 = bj * block_size;
            let j1 = ((bj + 1) * block_size).min(n);

            if bi == bj {
                // Within-block upper triangle (i < j).
                for i in i0..i1 {
                    if let Some(emb_i) = &embeddings[i] {
                        for j in (i + 1)..i1 {
                            if let Some(emb_j) = &embeddings[j] {
                                try_add_edge(
                                    i,
                                    j,
                                    file_ids,
                                    emb_i,
                                    emb_j,
                                    threshold,
                                    k_neighbors,
                                    &mut candidate_lists,
                                );
                            }
                        }
                    }
                }
            } else {
                // Cross-block: all pairs between blocks (i in I, j in J), i < j by construction.
                for i in i0..i1 {
                    if let Some(emb_i) = &embeddings[i] {
                        for j in j0..j1 {
                            if let Some(emb_j) = &embeddings[j] {
                                try_add_edge(
                                    i,
                                    j,
                                    file_ids,
                                    emb_i,
                                    emb_j,
                                    threshold,
                                    k_neighbors,
                                    &mut candidate_lists,
                                );
                            }
                        }
                    }
                }
            }

            block_pairs_done += 1;
            let current_pct = if total_block_pairs > 0 {
                block_pairs_done * 100 / total_block_pairs
            } else {
                100
            };
            if current_pct >= last_pct + 5 || block_pairs_done == total_block_pairs {
                progress_fn(ClusterProgress {
                    phase: "graph".to_string(),
                    current: current_pct,
                    total: 100,
                });
                last_pct = current_pct;
            }
        }
    }

    Ok(candidate_lists)
}

/// Approximate Top-K graph via pure-Rust HNSW (`instant-distance`).
///
/// Same edge meaning as exact: weight `(1-d)^2` when cosine distance `< threshold`,
/// same-file edges banned. On empty valid set returns empty lists. Cancel returns `Err("cancelled")`.
fn build_knn_graph_ann<P, C>(
    file_ids: &[i64],
    embeddings: &[Option<Vec<f32>>],
    threshold: f32,
    k_neighbors: usize,
    mut progress_fn: P,
    is_cancelled_fn: C,
) -> Result<Vec<Vec<(usize, f32)>>, String>
where
    P: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    let n = file_ids.len();
    debug_assert_eq!(n, embeddings.len());
    let mut candidate_lists: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
    if n == 0 {
        return Ok(candidate_lists);
    }
    if is_cancelled_fn() {
        return Err("cancelled".to_string());
    }

    // Compact only faces with valid embeddings for the index; map back via HnswMap values.
    let mut points: Vec<FacePoint> = Vec::with_capacity(n);
    let mut face_indices: Vec<usize> = Vec::with_capacity(n);
    for (i, emb) in embeddings.iter().enumerate() {
        if let Some(v) = emb {
            if v.is_empty() {
                continue;
            }
            points.push(FacePoint(Arc::from(v.as_slice())));
            face_indices.push(i);
        }
    }
    let m = points.len();
    if m == 0 {
        progress_fn(ClusterProgress {
            phase: "graph".to_string(),
            current: 100,
            total: 100,
        });
        return Ok(candidate_lists);
    }

    progress_fn(ClusterProgress {
        phase: "graph".to_string(),
        current: 0,
        total: 100,
    });
    if is_cancelled_fn() {
        return Err("cancelled".to_string());
    }

    let ef_search = t_common::CLUSTER_ANN_EF_SEARCH.max(k_neighbors.saturating_mul(3).max(64));
    let ef_construction = t_common::CLUSTER_ANN_EF_CONSTRUCTION.max(ef_search);
    // Deterministic seed so re-runs are more comparable in logs/tests.
    // Values = original face indices (HnswMap reorders them with PointIds).
    let hnsw_map = HnswBuilder::default()
        .ef_search(ef_search)
        .ef_construction(ef_construction)
        .seed(0xC1A5_FACE_u64)
        .build(points, face_indices.clone());

    if is_cancelled_fn() {
        return Err("cancelled".to_string());
    }
    progress_fn(ClusterProgress {
        phase: "graph".to_string(),
        current: 15,
        total: 100,
    });

    // Query more neighbors than K so same-file / weak hits can be filtered.
    let query_limit = k_neighbors.saturating_mul(3).max(k_neighbors + 8).min(m);
    let mut search = HnswSearch::default();
    let mut last_pct = 15usize;

    for (qi, &face_i) in face_indices.iter().enumerate() {
        if is_cancelled_fn() {
            return Err("cancelled".to_string());
        }

        let Some(emb_i) = embeddings[face_i].as_ref() else {
            continue;
        };
        let query = FacePoint(Arc::from(emb_i.as_slice()));
        let mut found = 0usize;
        for item in hnsw_map.search(&query, &mut search) {
            if found >= query_limit {
                break;
            }
            found += 1;
            let face_j = *item.value;
            if face_j == face_i {
                continue;
            }
            if face_j >= n || file_ids[face_i] == file_ids[face_j] {
                continue;
            }
            let dist = item.distance;
            if dist < threshold {
                let weight = 1.0 - dist;
                let adjusted_weight = weight * weight;
                // Dual insert matches exact path (undirected contribution from one discovery).
                insert_top_k(
                    &mut candidate_lists[face_i],
                    (face_j, adjusted_weight),
                    k_neighbors,
                );
                insert_top_k(
                    &mut candidate_lists[face_j],
                    (face_i, adjusted_weight),
                    k_neighbors,
                );
            }
        }

        let current_pct = 15 + (qi + 1) * 85 / m.max(1);
        if current_pct >= last_pct + 5 || qi + 1 == m {
            progress_fn(ClusterProgress {
                phase: "graph".to_string(),
                current: current_pct.min(100),
                total: 100,
            });
            last_pct = current_pct;
        }
    }

    Ok(candidate_lists)
}

/// Adaptive graph build driven by [`ClusterMode`].
/// Returns (candidate_lists, strategy_label): `exact` | `ann` | `blocked` (fallback).
fn build_knn_graph_adaptive<P, C>(
    file_ids: &[i64],
    embeddings: &[Option<Vec<f32>>],
    threshold: f32,
    k_neighbors: usize,
    mode: ClusterMode,
    mut progress_fn: P,
    is_cancelled_fn: C,
) -> Result<(Vec<Vec<(usize, f32)>>, &'static str), String>
where
    P: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    let n = file_ids.len();
    let preferred = graph_strategy_for_mode(n, mode);
    match preferred {
        "ann" => match build_knn_graph_ann(
            file_ids,
            embeddings,
            threshold,
            k_neighbors,
            &mut progress_fn,
            &is_cancelled_fn,
        ) {
            Ok(lists) => Ok((lists, "ann")),
            Err(e) if e.starts_with("cancelled") => Err(e),
            Err(e) => {
                eprintln!(
                    "[cluster] ann failed ({e}); falling back to blocked exact tiles"
                );
                let lists = build_knn_graph_blocked(
                    file_ids,
                    embeddings,
                    threshold,
                    k_neighbors,
                    t_common::CLUSTER_BLOCK_SIZE,
                    progress_fn,
                    is_cancelled_fn,
                )?;
                Ok((lists, "blocked"))
            }
        },
        "blocked" => {
            let lists = build_knn_graph_blocked(
                file_ids,
                embeddings,
                threshold,
                k_neighbors,
                t_common::CLUSTER_BLOCK_SIZE,
                progress_fn,
                is_cancelled_fn,
            )?;
            Ok((lists, "blocked"))
        }
        _ => {
            let lists = build_knn_graph_exact(
                file_ids,
                embeddings,
                threshold,
                k_neighbors,
                progress_fn,
                is_cancelled_fn,
            )?;
            Ok((lists, "exact"))
        }
    }
}

/// Compare two Top-K graphs for quality gates (exact vs blocked / ANN).
///
/// Reports per-node neighbor set Jaccard and aggregate edge-set overlap. Tests only.
#[cfg(test)]
fn graph_edge_parity_report(
    reference: &[Vec<(usize, f32)>],
    other: &[Vec<(usize, f32)>],
) -> GraphParityReport {
    assert_eq!(reference.len(), other.len());
    let n = reference.len();
    let mut jaccard_sum = 0.0f64;
    let mut nodes_equal = 0usize;
    let mut ref_edges = 0usize;
    let mut other_edges = 0usize;

    // Undirected edge sets (min,max) for global overlap.
    let mut ref_set = std::collections::HashSet::new();
    let mut other_set = std::collections::HashSet::new();

    for i in 0..n {
        let a: std::collections::HashSet<usize> = reference[i].iter().map(|&(t, _)| t).collect();
        let b: std::collections::HashSet<usize> = other[i].iter().map(|&(t, _)| t).collect();
        ref_edges += a.len();
        other_edges += b.len();
        let inter = a.intersection(&b).count();
        let union = a.union(&b).count();
        let j = if union == 0 {
            1.0
        } else {
            inter as f64 / union as f64
        };
        jaccard_sum += j;
        if a == b {
            nodes_equal += 1;
        }
        for &t in &a {
            if i < t {
                ref_set.insert((i, t));
            } else {
                ref_set.insert((t, i));
            }
        }
        for &t in &b {
            if i < t {
                other_set.insert((i, t));
            } else {
                other_set.insert((t, i));
            }
        }
    }
    let shared_undirected = ref_set.intersection(&other_set).count();
    let ref_u = ref_set.len();
    let other_u = other_set.len();
    let only_ref = ref_u.saturating_sub(shared_undirected);
    let only_other = other_u.saturating_sub(shared_undirected);

    GraphParityReport {
        n,
        mean_neighbor_jaccard: if n == 0 {
            1.0
        } else {
            jaccard_sum / n as f64
        },
        nodes_identical: nodes_equal,
        directed_edge_count_ref: ref_edges,
        directed_edge_count_other: other_edges,
        undirected_shared: shared_undirected,
        undirected_only_ref: only_ref,
        undirected_only_other: only_other,
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct GraphParityReport {
    n: usize,
    mean_neighbor_jaccard: f64,
    nodes_identical: usize,
    directed_edge_count_ref: usize,
    directed_edge_count_other: usize,
    undirected_shared: usize,
    undirected_only_ref: usize,
    undirected_only_other: usize,
}

#[cfg(test)]
impl GraphParityReport {
    fn summary_line(&self) -> String {
        format!(
            "n={} mean_jaccard={:.4} nodes_identical={}/{} dir_edges ref/other={}/{} undirected shared={} only_ref={} only_other={}",
            self.n,
            self.mean_neighbor_jaccard,
            self.nodes_identical,
            self.n,
            self.directed_edge_count_ref,
            self.directed_edge_count_other,
            self.undirected_shared,
            self.undirected_only_ref,
            self.undirected_only_other
        )
    }
}

/// Calculate cosine distance between two PRE-PARSED embeddings
/// Distance = 1.0 - Cosine Similarity
/// NOTE: Assumes input vectors are already normalized!
fn cosine_distance(emb1: &[f32], emb2: &[f32]) -> f32 {
    debug_assert_eq!(
        emb1.len(),
        emb2.len(),
        "embedding length mismatch: {} vs {}",
        emb1.len(),
        emb2.len()
    );
    // Dot product of normalized vectors = cosine similarity
    let n = emb1.len().min(emb2.len());
    let mut dot_product = 0.0f32;
    for i in 0..n {
        dot_product += emb1[i] * emb2[i];
    }

    // Clamp similarity to [-1.0, 1.0] to handle floating point errors
    let similarity = dot_product.clamp(-1.0, 1.0);
    1.0 - similarity
}

/// Helper: Parse raw byte embedding to normalized f32 vector
fn parse_embedding(bytes: &[u8]) -> Option<Vec<f32>> {
    // Embedding bytes should be tightly packed f32 values.
    // If not, skip this embedding instead of panicking.
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return None;
    }
    let emb_vec: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|chunk| {
            let arr = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect();

    // Normalize
    let norm: f32 = emb_vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        Some(emb_vec.iter().map(|x| x / norm).collect())
    } else {
        None
    }
}

/// Edge in the similarity graph
#[derive(Clone)]
struct Edge {
    to: usize,   // Target node index
    weight: f32, // Edge weight (similarity = 1 - distance)
}

/// Insert edge into candidate list, keeping at most `max_edges` by weight (descending).
/// Linear insert (K≈80) — avoids full O(K log K) sort on every pair.
fn insert_top_k(candidates: &mut Vec<(usize, f32)>, edge: (usize, f32), max_edges: usize) {
    if max_edges == 0 {
        return;
    }
    if candidates.len() < max_edges {
        // Insert keeping sorted desc by weight.
        let mut i = candidates.len();
        candidates.push(edge);
        while i > 0 && candidates[i].1 > candidates[i - 1].1 {
            candidates.swap(i, i - 1);
            i -= 1;
        }
        return;
    }
    // Full: drop if not better than weakest (last).
    let min_weight = candidates.last().map(|e| e.1).unwrap_or(f32::NEG_INFINITY);
    if edge.1 <= min_weight {
        return;
    }
    candidates.pop();
    let mut i = candidates.len();
    candidates.push(edge);
    while i > 0 && candidates[i].1 > candidates[i - 1].1 {
        candidates.swap(i, i - 1);
        i -= 1;
    }
}

/// Run Chinese Whispers clustering while preserving existing person assignments.
///
/// Strategy (incremental / seed-preserving):
/// 1. Keep all existing person rows and face→person links.
/// 2. Cluster ALL faces (assigned + unassigned) so new faces can join existing people.
/// 3. Seed labels from existing person_id when present; only reassign unassigned faces.
/// 4. Create new Person rows only for clusters made entirely of previously unassigned faces.
///
/// Memory-optimized:
/// 1. Uses slim face data (id, file_id, person_id, embedding_bytes)
/// 2. Prunes candidate edges to Top-K during build (not after), bounding memory at N * K_NEIGHBORS
/// 3. Pre-parses all embeddings once to avoid allocations in inner loop
pub fn cluster_faces<F, C>(
    threshold: f32,
    mode: ClusterMode,
    mut progress_fn: F,
    is_cancelled_fn: C,
) -> Result<usize, String>
where
    F: FnMut(ClusterProgress),
    C: Fn() -> bool,
{
    const K_NEIGHBORS: usize = t_common::K_NEIGHBORS;
    let cluster_start = Instant::now();

    // 1. Load ALL faces — slim: (face_id, file_id, person_id, embedding_bytes)
    //    Do NOT wipe existing assignments; user renames / merges must survive re-index.
    let mut slim_faces = Face::get_all_for_clustering()?;
    let n = slim_faces.len();
    if n == 0 {
        eprintln!("[cluster] start n=0 (no faces)");
        return Ok(0);
    }

    let pre_assigned = slim_faces.iter().filter(|f| f.2.is_some()).count();
    let pre_unassigned = n - pre_assigned;
    let load_ms = cluster_start.elapsed().as_millis();
    let strategy = graph_strategy_for_mode(n, mode);
    eprintln!(
        "[cluster] start n={} pre_assigned={} pre_unassigned={} threshold={:.4} k={} mode={} strategy={} n_exact={} block={} load_ms={}",
        n,
        pre_assigned,
        pre_unassigned,
        threshold,
        K_NEIGHBORS,
        mode.as_str(),
        strategy,
        t_common::CLUSTER_N_EXACT,
        t_common::CLUSTER_BLOCK_SIZE,
        load_ms
    );

    // 2. Pre-parse embeddings (do this once)
    let parse_start = Instant::now();
    let mut parsed_embeddings: Vec<Option<Vec<f32>>> = Vec::with_capacity(n);
    let mut file_ids: Vec<i64> = Vec::with_capacity(n);
    let mut parse_ok = 0usize;
    for (_id, file_id, _person_id, embedding_bytes) in &mut slim_faces {
        file_ids.push(*file_id);
        let emb = embedding_bytes.as_deref().and_then(parse_embedding);
        if emb.is_some() {
            parse_ok += 1;
        }
        parsed_embeddings.push(emb);
        embedding_bytes.take();
    }
    let parse_ms = parse_start.elapsed().as_millis();
    eprintln!(
        "[cluster] parse embeddings ok={}/{} parse_ms={}",
        parse_ok, n, parse_ms
    );

    // 3. Build K-NN Graph with early Top-K pruning (adaptive exact vs blocked exact)
    //    candidate_lists memory is bounded at N * K_NEIGHBORS entries
    let graph_start = Instant::now();
    let (candidate_lists, graph_strategy) = build_knn_graph_adaptive(
        &file_ids,
        &parsed_embeddings,
        threshold,
        K_NEIGHBORS,
        mode,
        &mut progress_fn,
        &is_cancelled_fn,
    )?;
    let graph_ms = graph_start.elapsed().as_millis();
    let edge_count: usize = candidate_lists.iter().map(|c| c.len()).sum();
    eprintln!(
        "[cluster] graph_{} done n={} edges={} (directed half-entries) graph_ms={}",
        graph_strategy, n, edge_count, graph_ms
    );

    // 4. Build final graph from pruned candidate lists (edges already Top-K)
    let mut graph: Vec<Vec<Edge>> = vec![Vec::new(); n];

    for (i, candidates) in candidate_lists.iter().enumerate() {
        for &(to, weight) in candidates {
            graph[i].push(Edge { to, weight });
        }
    }

    // Free graph-building allocations before Chinese Whispers
    drop(candidate_lists);
    drop(parsed_embeddings);
    drop(file_ids);

    // 5. Seed labels: faces that already belong to a person share a label keyed by person_id.
    //    Unassigned faces start with unique labels so they can join via whispers.
    let mut person_label: HashMap<i64, usize> = HashMap::new();
    let mut labels: Vec<usize> = Vec::with_capacity(n);
    let mut next_label = 0usize;
    for face in &slim_faces {
        if let Some(pid) = face.2 {
            let label = *person_label.entry(pid).or_insert_with(|| {
                let l = next_label;
                next_label += 1;
                l
            });
            labels.push(label);
        } else {
            labels.push(next_label);
            next_label += 1;
        }
    }

    // Frozen labels: existing assignments must not be reassigned by clustering.
    let frozen: Vec<bool> = slim_faces.iter().map(|f| f.2.is_some()).collect();

    // 6. Run Chinese Whispers Algorithm (only moves unassigned faces)
    let whisper_start = Instant::now();
    let mut order: Vec<usize> = (0..n).collect();
    let mut rng = rand::thread_rng();
    let max_iterations = 20;
    let mut iterations_run = 0usize;

    for iter in 0..max_iterations {
        // Check for cancellation
        if is_cancelled_fn() {
            eprintln!(
                "[cluster] cancelled during whisper iter={} whisper_ms={}",
                iter + 1,
                whisper_start.elapsed().as_millis()
            );
            return Err("cancelled".to_string());
        }

        let mut changed = false;
        iterations_run = iter + 1;

        progress_fn(ClusterProgress {
            phase: "iterate".to_string(),
            current: iter + 1,
            total: max_iterations,
        });

        order.shuffle(&mut rng);

        for &node in &order {
            if frozen[node] || graph[node].is_empty() {
                continue;
            }

            // Count weighted votes
            let mut label_weights: HashMap<usize, f32> = HashMap::new();
            for edge in &graph[node] {
                let neighbor_label = labels[edge.to];
                *label_weights.entry(neighbor_label).or_insert(0.0) += edge.weight;
            }

            // Find best label
            let best_label = label_weights
                .into_iter()
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(label, _)| label)
                .unwrap_or(labels[node]);

            if labels[node] != best_label {
                labels[node] = best_label;
                changed = true;
            }
        }

        if !changed {
            progress_fn(ClusterProgress {
                phase: "converged".to_string(),
                current: iter + 1,
                total: max_iterations,
            });
            break;
        }
    }
    let whisper_ms = whisper_start.elapsed().as_millis();
    eprintln!(
        "[cluster] whisper done iterations={} whisper_ms={}",
        iterations_run, whisper_ms
    );

    // Check for cancellation before assignment
    if is_cancelled_fn() {
        eprintln!("[cluster] cancelled before assign total_ms={}", cluster_start.elapsed().as_millis());
        return Err("cancelled".to_string());
    }

    // 7. Collect clusters
    let mut cluster_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for (i, &label) in labels.iter().enumerate() {
        cluster_map.entry(label).or_default().push(i);
    }
    drop(graph);
    drop(order);

    // Reverse map: label -> existing person_id (if any seed face carried one)
    let label_to_person: HashMap<usize, i64> = person_label
        .into_iter()
        .map(|(pid, label)| (label, pid))
        .collect();

    // 8. Filter clusters
    const MIN_SAMPLES: usize = t_common::MIN_SAMPLES;
    let valid_clusters: Vec<_> = cluster_map
        .into_iter()
        .filter(|(_, face_indices)| face_indices.len() >= MIN_SAMPLES)
        .collect();

    let total_clusters = valid_clusters.len();

    // 9. Assign only previously-unassigned faces
    let assign_start = Instant::now();
    let mut total_assigned = 0usize;
    let mut next_person_num = Face::next_auto_person_number()?;

    for (cluster_idx, (label, cluster_face_indices)) in valid_clusters.into_iter().enumerate() {
        if is_cancelled_fn() {
            // Partial assigns already committed; surface cancel so UI does not look like full success.
            eprintln!(
                "[cluster] cancelled mid-assign assigned={} clusters_seen={}/{} assign_ms={} total_ms={}",
                total_assigned,
                cluster_idx,
                total_clusters,
                assign_start.elapsed().as_millis(),
                cluster_start.elapsed().as_millis()
            );
            return Err(format!("cancelled after assigning {total_assigned} faces"));
        }

        progress_fn(ClusterProgress {
            phase: "assign".to_string(),
            current: cluster_idx + 1,
            total: total_clusters,
        });

        // Prefer an existing person if any face in this cluster already belongs to one.
        let mut person_id = label_to_person.get(&label).copied();
        if person_id.is_none() {
            for &face_idx in &cluster_face_indices {
                if let Some(pid) = slim_faces[face_idx].2 {
                    person_id = Some(pid);
                    break;
                }
            }
        }

        let person_id = if let Some(pid) = person_id {
            pid
        } else {
            // Brand-new cluster of only unassigned faces → create a person.
            let person_name = format!("Person {}", next_person_num);
            next_person_num += 1;
            Person::create(Some(&person_name))?
        };

        for face_idx in cluster_face_indices {
            // Never overwrite an existing assignment (handles multi-person seeds in one label).
            if slim_faces[face_idx].2.is_some() {
                continue;
            }
            Face::assign_to_person(slim_faces[face_idx].0, person_id)?;
            total_assigned += 1;
        }
    }
    let assign_ms = assign_start.elapsed().as_millis();

    drop(slim_faces);
    drop(labels);
    drop(frozen);

    // 10. Generate thumbnails
    let thumb_start = Instant::now();
    progress_fn(ClusterProgress {
        phase: "thumbnail".to_string(),
        current: 0,
        total: total_clusters,
    });

    Person::update_all_thumbnails()?;

    progress_fn(ClusterProgress {
        phase: "thumbnail".to_string(),
        current: total_clusters,
        total: total_clusters,
    });
    let thumb_ms = thumb_start.elapsed().as_millis();
    let total_ms = cluster_start.elapsed().as_millis();
    eprintln!(
        "[cluster] done n={} pre_assigned={} pre_unassigned={} newly_assigned={} valid_clusters={} graph_ms={} whisper_ms={} assign_ms={} thumb_ms={} total_ms={}",
        n,
        pre_assigned,
        pre_unassigned,
        total_assigned,
        total_clusters,
        graph_ms,
        whisper_ms,
        assign_ms,
        thumb_ms,
        total_ms
    );

    Ok(total_assigned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::time::Instant;

    /// L2-normalize a mutable vector (matches parse_embedding normalization).
    fn l2_normalize(v: &mut [f32]) {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in v.iter_mut() {
                *x /= norm;
            }
        }
    }

    /// Synthetic unit vectors: `clusters` groups of size `per_cluster`, dim=d, plus noise.
    fn synthetic_unit_vectors(
        clusters: usize,
        per_cluster: usize,
        dim: usize,
        seed: u64,
    ) -> (Vec<i64>, Vec<Option<Vec<f32>>>) {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let n = clusters * per_cluster;
        let mut file_ids = Vec::with_capacity(n);
        let mut embeddings = Vec::with_capacity(n);

        for c in 0..clusters {
            // Cluster center: one-hot-ish random direction
            let mut center = vec![0.0f32; dim];
            for x in center.iter_mut() {
                *x = rng.gen_range(-1.0..1.0);
            }
            l2_normalize(&mut center);

            for k in 0..per_cluster {
                let mut emb = center.clone();
                for x in emb.iter_mut() {
                    *x += rng.gen_range(-0.05..0.05);
                }
                l2_normalize(&mut emb);
                // Distinct file_id per face so same-file ban does not drop true neighbors.
                file_ids.push((c * per_cluster + k) as i64 + 1);
                embeddings.push(Some(emb));
            }
        }
        (file_ids, embeddings)
    }

    #[test]
    fn insert_top_k_keeps_highest_weights() {
        let mut c = Vec::new();
        insert_top_k(&mut c, (0, 0.1), 3);
        insert_top_k(&mut c, (1, 0.9), 3);
        insert_top_k(&mut c, (2, 0.5), 3);
        insert_top_k(&mut c, (3, 0.2), 3); // should replace 0.1
        insert_top_k(&mut c, (4, 0.05), 3); // dropped
        assert_eq!(c.len(), 3);
        assert!(c.iter().all(|&(_, w)| w >= 0.2));
        assert_eq!(c[0].1, 0.9);
    }

    #[test]
    fn exact_graph_same_file_ban() {
        // Two identical embeddings, same file_id → no edge.
        let emb = {
            let mut v = vec![1.0f32, 0.0, 0.0, 0.0];
            l2_normalize(&mut v);
            v
        };
        let file_ids = vec![1i64, 1];
        let embeddings = vec![Some(emb.clone()), Some(emb)];
        let lists = build_knn_graph_exact(
            &file_ids,
            &embeddings,
            0.5,
            10,
            |_| {},
            || false,
        )
        .unwrap();
        assert!(lists[0].is_empty());
        assert!(lists[1].is_empty());
    }

    #[test]
    fn exact_graph_cancel_returns_err() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (file_ids, embeddings) = synthetic_unit_vectors(2, 8, 32, 1);
        let calls = AtomicUsize::new(0);
        let result = build_knn_graph_exact(
            &file_ids,
            &embeddings,
            0.4,
            5,
            |_| {},
            || calls.fetch_add(1, Ordering::Relaxed) > 0,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "cancelled");
    }

    /// Small-sample wall-time baseline for exact graph (P0 measurement).
    /// Prints ms for n≈128 / n≈512 synthetic unit vectors; not a hard CI gate.
    #[test]
    fn bench_exact_graph_small_synthetic() {
        // Quiet progress; threshold loose so many edges form (worst-ish Top-K work).
        let threshold = 0.6f32;
        let k = t_common::K_NEIGHBORS;

        for &(clusters, per) in &[(8usize, 16usize), (16, 32)] {
            let n = clusters * per;
            let (file_ids, embeddings) = synthetic_unit_vectors(clusters, per, 512, 42);
            let t0 = Instant::now();
            let lists = build_knn_graph_exact(
                &file_ids,
                &embeddings,
                threshold,
                k,
                |_| {},
                || false,
            )
            .expect("graph build");
            let ms = t0.elapsed().as_millis();
            let edges: usize = lists.iter().map(|c| c.len()).sum();
            eprintln!(
                "[cluster-bench] exact_graph n={} dim=512 k={} edges={} ms={}",
                n, k, edges, ms
            );
            // Sanity: each node should keep at most k neighbors.
            assert!(lists.iter().all(|c| c.len() <= k));
            // With synthetic tight clusters + loose threshold, expect non-empty graph.
            assert!(edges > 0, "expected edges for n={n}");
        }
    }

    #[test]
    fn cosine_distance_identical_is_zero() {
        let mut a = vec![0.3f32, -0.1, 0.7, 0.2];
        l2_normalize(&mut a);
        let d = cosine_distance(&a, &a);
        assert!(d.abs() < 1e-5, "dist={d}");
    }

    /// Normalize neighbor lists for set-equality (order can differ between strategies).
    fn neighbor_sets(lists: &[Vec<(usize, f32)>]) -> Vec<Vec<(usize, u32)>> {
        lists
            .iter()
            .map(|c| {
                let mut v: Vec<(usize, u32)> = c
                    .iter()
                    .map(|&(to, w)| (to, w.to_bits()))
                    .collect();
                v.sort_by_key(|&(to, bits)| (to, bits));
                v
            })
            .collect()
    }

    #[test]
    fn blocked_matches_exact_on_synthetic() {
        let (file_ids, embeddings) = synthetic_unit_vectors(6, 12, 64, 7);
        let n = file_ids.len();
        let threshold = 0.55f32;
        // k = n keeps every qualifying edge so equal-weight Top-K insert order cannot diverge.
        let k = n;
        let exact = build_knn_graph_exact(
            &file_ids,
            &embeddings,
            threshold,
            k,
            |_| {},
            || false,
        )
        .unwrap();
        // Force multi-block path with tiny tiles.
        let blocked = build_knn_graph_blocked(
            &file_ids,
            &embeddings,
            threshold,
            k,
            8,
            |_| {},
            || false,
        )
        .unwrap();
        assert_eq!(
            neighbor_sets(&exact),
            neighbor_sets(&blocked),
            "blocked tiles must match exact edges (no Top-K drop)"
        );

        // Also with tight Top-K: edge *counts* per node should match even if rare equal-weight
        // ties could reorder which neighbor wins (strict > in insert_top_k).
        let k_tight = 15usize;
        let exact_k = build_knn_graph_exact(
            &file_ids,
            &embeddings,
            threshold,
            k_tight,
            |_| {},
            || false,
        )
        .unwrap();
        let blocked_k = build_knn_graph_blocked(
            &file_ids,
            &embeddings,
            threshold,
            k_tight,
            8,
            |_| {},
            || false,
        )
        .unwrap();
        for i in 0..n {
            assert_eq!(
                exact_k[i].len(),
                blocked_k[i].len(),
                "node {i} degree mismatch under Top-K"
            );
        }
    }

    #[test]
    fn blocked_same_file_ban() {
        let emb = {
            let mut v = vec![1.0f32, 0.0, 0.0, 0.0];
            l2_normalize(&mut v);
            v
        };
        let file_ids = vec![1i64, 1, 2];
        let embeddings = vec![Some(emb.clone()), Some(emb.clone()), Some(emb)];
        let lists = build_knn_graph_blocked(
            &file_ids,
            &embeddings,
            0.5,
            10,
            2,
            |_| {},
            || false,
        )
        .unwrap();
        // Indices 0 and 1 share file_id → no edge between them.
        assert!(!lists[0].iter().any(|&(to, _)| to == 1));
        assert!(!lists[1].iter().any(|&(to, _)| to == 0));
        // 0–2 and 1–2 may connect (identical emb, different files).
        assert!(lists[0].iter().any(|&(to, _)| to == 2));
        assert!(lists[2].iter().any(|&(to, _)| to == 0));
    }

    #[test]
    fn blocked_cancel_returns_err() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (file_ids, embeddings) = synthetic_unit_vectors(4, 8, 32, 3);
        let calls = AtomicUsize::new(0);
        let result = build_knn_graph_blocked(
            &file_ids,
            &embeddings,
            0.4,
            5,
            4,
            |_| {},
            || calls.fetch_add(1, Ordering::Relaxed) > 0,
        );
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "cancelled");
    }

    #[test]
    fn adaptive_picks_exact_below_n_exact() {
        assert_eq!(graph_strategy_for_mode(0, ClusterMode::Auto), "exact");
        assert_eq!(
            graph_strategy_for_mode(t_common::CLUSTER_N_EXACT - 1, ClusterMode::Auto),
            "exact"
        );
        assert_eq!(
            graph_strategy_for_mode(t_common::CLUSTER_N_EXACT, ClusterMode::Auto),
            "ann"
        );
        assert_eq!(
            graph_strategy_for_mode(t_common::CLUSTER_N_EXACT + 1, ClusterMode::Auto),
            "ann"
        );
        assert_eq!(
            graph_strategy_for_mode(t_common::CLUSTER_N_EXACT + 1, ClusterMode::Exact),
            "exact"
        );
        assert_eq!(graph_strategy_for_mode(1, ClusterMode::Fast), "ann");
    }

    #[test]
    fn cluster_mode_parse() {
        assert_eq!(ClusterMode::parse(None), ClusterMode::Auto);
        assert_eq!(ClusterMode::parse(Some("AUTO")), ClusterMode::Auto);
        assert_eq!(ClusterMode::parse(Some("exact")), ClusterMode::Exact);
        assert_eq!(ClusterMode::parse(Some("fast")), ClusterMode::Fast);
        assert_eq!(ClusterMode::parse(Some("nope")), ClusterMode::Auto);
    }

    #[test]
    fn adaptive_dispatch_small_n_uses_exact() {
        let (file_ids, embeddings) = synthetic_unit_vectors(4, 8, 32, 11);
        let (lists, strategy) = build_knn_graph_adaptive(
            &file_ids,
            &embeddings,
            0.5,
            10,
            ClusterMode::Auto,
            |_| {},
            || false,
        )
        .unwrap();
        assert_eq!(strategy, "exact");
        assert_eq!(lists.len(), file_ids.len());

        let (_, fast_strategy) = build_knn_graph_adaptive(
            &file_ids,
            &embeddings,
            0.5,
            10,
            ClusterMode::Fast,
            |_| {},
            || false,
        )
        .unwrap();
        assert_eq!(fast_strategy, "ann");
    }

    #[test]
    fn ann_graph_produces_edges_and_respects_same_file() {
        let (mut file_ids, embeddings) = synthetic_unit_vectors(4, 16, 64, 5);
        // Force first two faces same file.
        file_ids[0] = 1;
        file_ids[1] = 1;
        let lists = build_knn_graph_ann(
            &file_ids,
            &embeddings,
            0.55,
            20,
            |_| {},
            || false,
        )
        .unwrap();
        let edges: usize = lists.iter().map(|c| c.len()).sum();
        assert!(edges > 0, "ANN should find neighbors in synthetic clusters");
        assert!(!lists[0].iter().any(|&(to, _)| to == 1));
        assert!(!lists[1].iter().any(|&(to, _)| to == 0));
    }

    #[test]
    fn ann_vs_exact_parity_soft_gate() {
        // Small synthetic: ANN should recover most exact neighbors (not bit-identical).
        let (file_ids, embeddings) = synthetic_unit_vectors(6, 20, 128, 17);
        let threshold = 0.5f32;
        let k = 30usize;
        let exact = build_knn_graph_exact(
            &file_ids,
            &embeddings,
            threshold,
            k,
            |_| {},
            || false,
        )
        .unwrap();
        let ann = build_knn_graph_ann(
            &file_ids,
            &embeddings,
            threshold,
            k,
            |_| {},
            || false,
        )
        .unwrap();
        let report = graph_edge_parity_report(&exact, &ann);
        eprintln!("[cluster-parity-ann] {}", report.summary_line());
        // Soft quality floor for in-process HNSW on tight synthetic clusters.
        assert!(
            report.mean_neighbor_jaccard >= 0.5,
            "ANN recall too low vs exact: {}",
            report.summary_line()
        );
        assert!(report.directed_edge_count_other > 0);
    }

    /// Dev quality gate: print exact-vs-blocked edge parity on synthetic sample (P2).
    /// Not a hard CI fail unless mean Jaccard drops below 0.99 (should be 1.0 for full-k).
    #[test]
    fn report_exact_vs_blocked_edge_parity() {
        let (file_ids, embeddings) = synthetic_unit_vectors(8, 16, 128, 99);
        let n = file_ids.len();
        let threshold = 0.5f32;
        let k = t_common::K_NEIGHBORS.min(n);

        let exact = build_knn_graph_exact(
            &file_ids,
            &embeddings,
            threshold,
            k,
            |_| {},
            || false,
        )
        .unwrap();
        let blocked = build_knn_graph_blocked(
            &file_ids,
            &embeddings,
            threshold,
            k,
            16,
            |_| {},
            || false,
        )
        .unwrap();
        let report = graph_edge_parity_report(&exact, &blocked);
        eprintln!("[cluster-parity] {}", report.summary_line());
        // Blocked is exact-distance; Jaccard should be ~1. Equal-weight Top-K ties may
        // slightly diverge under tight k — keep a soft floor.
        assert!(
            report.mean_neighbor_jaccard >= 0.99,
            "unexpected exact/blocked divergence: {}",
            report.summary_line()
        );
    }
}
