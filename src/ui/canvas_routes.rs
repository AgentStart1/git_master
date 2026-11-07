use super::commit_canvas::{CanvasEdge, CanvasEdgeKind, CanvasNodeId, CanvasPoint};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

const CLEARANCE: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Geometry-only cache: viewport changes do not invalidate world-coordinate routes.
#[derive(Debug, Default)]
pub struct RouteCache {
    cards: HashMap<CanvasNodeId, CardRect>,
    edges: Vec<CanvasEdge>,
    routes: std::rc::Rc<Vec<Vec<CanvasPoint>>>,
}
impl RouteCache {
    pub fn routes(
        &mut self,
        edges: &[CanvasEdge],
        cards: HashMap<CanvasNodeId, CardRect>,
    ) -> std::rc::Rc<Vec<Vec<CanvasPoint>>> {
        if self.cards != cards || self.edges != edges {
            self.routes =
                std::rc::Rc::new(edges.iter().map(|edge| edge_route(edge, &cards)).collect());
            self.cards = cards;
            self.edges = edges.to_vec();
        }
        self.routes.clone()
    }
}
impl CardRect {
    pub fn new(p: CanvasPoint, width: f32, height: f32) -> Self {
        Self {
            left: p.x,
            top: p.y,
            right: p.x + width,
            bottom: p.y + height,
        }
    }
    fn blocks(self, a: CanvasPoint, b: CanvasPoint) -> bool {
        if (a.x - b.x).abs() < 0.01 {
            a.x > self.left
                && a.x < self.right
                && a.y.min(b.y) < self.bottom
                && a.y.max(b.y) > self.top
        } else {
            a.y > self.top
                && a.y < self.bottom
                && a.x.min(b.x) < self.right
                && a.x.max(b.x) > self.left
        }
    }
}

/// World-coordinate polylines are shared by the painter and read-only diagnostics.
/// Endpoints lie on card boundaries; no visible ports are needed.
pub fn edge_route(edge: &CanvasEdge, cards: &HashMap<CanvasNodeId, CardRect>) -> Vec<CanvasPoint> {
    let (Some(from), Some(to)) = (cards.get(&edge.from), cards.get(&edge.to)) else {
        return Vec::new();
    };
    let (start, exit, entry, end) = match edge.kind {
        CanvasEdgeKind::Reference => {
            let y = (from.top + from.bottom) / 2.0;
            let target_y = (to.top + to.bottom) / 2.0;
            if (from.left + from.right) >= (to.left + to.right) {
                // The default label/commit gap is narrower than two clearances.
                // Meet in the gap instead of overshooting and doubling back.
                let clearance = ((from.left - to.right) / 2.0).clamp(0.0, CLEARANCE);
                (
                    CanvasPoint::new(from.left, y),
                    CanvasPoint::new(from.left - clearance, y),
                    CanvasPoint::new(to.right + clearance, target_y),
                    CanvasPoint::new(to.right, target_y),
                )
            } else {
                let clearance = ((to.left - from.right) / 2.0).clamp(0.0, CLEARANCE);
                (
                    CanvasPoint::new(from.right, y),
                    CanvasPoint::new(from.right + clearance, y),
                    CanvasPoint::new(to.left - clearance, target_y),
                    CanvasPoint::new(to.left, target_y),
                )
            }
        }
        CanvasEdgeKind::Parent => {
            let x = (from.left + from.right) / 2.0;
            let target_x = (to.left + to.right) / 2.0;
            (
                CanvasPoint::new(x, from.bottom),
                CanvasPoint::new(x, from.bottom + CLEARANCE),
                CanvasPoint::new(target_x, to.top - CLEARANCE),
                CanvasPoint::new(target_x, to.top),
            )
        }
        CanvasEdgeKind::Submodule => {
            let y = (from.top + from.bottom) / 2.0;
            let target_y = (to.top + to.bottom) / 2.0;
            (
                CanvasPoint::new(from.right, y),
                CanvasPoint::new(from.right + CLEARANCE, y),
                CanvasPoint::new(to.left - CLEARANCE, target_y),
                CanvasPoint::new(to.left, target_y),
            )
        }
    };
    let obstacles = cards.values().copied().collect::<Vec<_>>();
    let preferred = if edge.kind == CanvasEdgeKind::Submodule && entry.x > exit.x {
        // Opposite-direction gitlinks must not collapse into one shared stem.
        // Stable channels also keep routes from shuffling when history appends.
        let key = edge
            .from
            .commit_id
            .bytes()
            .chain(edge.to.commit_id.bytes())
            .fold(0u32, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(byte as u32)
            });
        let offset = (8.0 + (key % 4) as f32 * 4.0) * if exit.y < entry.y { -1.0 } else { 1.0 };
        let x = (exit.x + entry.x) / 2.0 + offset;
        let path = vec![
            exit,
            CanvasPoint::new(x, exit.y),
            CanvasPoint::new(x, entry.y),
            entry,
        ];
        clear(&path, &obstacles).then_some(path)
    } else {
        None
    };
    let middle = preferred.unwrap_or_else(|| route_between(exit, entry, &obstacles));
    simplify(
        std::iter::once(start)
            .chain(middle)
            .chain(std::iter::once(end))
            .collect(),
    )
}
fn clear(points: &[CanvasPoint], obstacles: &[CardRect]) -> bool {
    points
        .windows(2)
        .all(|pair| !obstacles.iter().any(|r| r.blocks(pair[0], pair[1])))
}
fn distance(a: CanvasPoint, b: CanvasPoint) -> f32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
fn coordinates(values: impl Iterator<Item = f32>) -> Vec<f32> {
    let mut result = values.collect::<Vec<_>>();
    result.sort_by(f32::total_cmp);
    result.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    result
}
fn route_between(start: CanvasPoint, end: CanvasPoint, obstacles: &[CardRect]) -> Vec<CanvasPoint> {
    for points in [
        vec![start, CanvasPoint::new(start.x, end.y), end],
        vec![start, CanvasPoint::new(end.x, start.y), end],
    ] {
        if clear(&points, obstacles) {
            return simplify(points);
        }
    }
    let xs = coordinates(
        std::iter::once(start.x)
            .chain(std::iter::once(end.x))
            .chain(
                obstacles
                    .iter()
                    .flat_map(|r| [r.left - CLEARANCE, r.right + CLEARANCE]),
            ),
    );
    let ys = coordinates(
        std::iter::once(start.y)
            .chain(std::iter::once(end.y))
            .chain(
                obstacles
                    .iter()
                    .flat_map(|r| [r.top - CLEARANCE, r.bottom + CLEARANCE]),
            ),
    );
    let mut candidates = xs
        .iter()
        .map(|x| {
            vec![
                start,
                CanvasPoint::new(*x, start.y),
                CanvasPoint::new(*x, end.y),
                end,
            ]
        })
        .chain(ys.iter().map(|y| {
            vec![
                start,
                CanvasPoint::new(start.x, *y),
                CanvasPoint::new(end.x, *y),
                end,
            ]
        }))
        .collect::<Vec<_>>();
    candidates.sort_by(|a, b| {
        let length = |p: &[CanvasPoint]| p.windows(2).map(|w| distance(w[0], w[1])).sum::<f32>();
        length(a).total_cmp(&length(b))
    });
    if let Some(path) = candidates.into_iter().find(|p| clear(p, obstacles)) {
        return simplify(path);
    }
    // Only staggered obstacles require searching the rectilinear visibility grid.
    grid_route(start, end, &xs, &ys, obstacles).unwrap_or_else(|| {
        // Overlapping user-placed cards may cover an attachment. Retain the Git
        // relation behind those cards rather than deleting a real graph edge.
        vec![start, CanvasPoint::new(start.x, end.y), end]
    })
}

#[derive(Clone, Copy)]
struct Visit {
    estimate: f32,
    distance: f32,
    node: usize,
}
impl PartialEq for Visit {
    fn eq(&self, other: &Self) -> bool {
        self.node == other.node && self.estimate == other.estimate
    }
}
impl Eq for Visit {}
impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .estimate
            .total_cmp(&self.estimate)
            .then_with(|| other.node.cmp(&self.node))
    }
}
fn grid_route(
    start: CanvasPoint,
    end: CanvasPoint,
    xs: &[f32],
    ys: &[f32],
    obstacles: &[CardRect],
) -> Option<Vec<CanvasPoint>> {
    let index = |p: CanvasPoint| {
        xs.iter().position(|x| (*x - p.x).abs() < 0.01).unwrap()
            + ys.iter().position(|y| (*y - p.y).abs() < 0.01).unwrap() * xs.len()
    };
    let point = |index: usize| CanvasPoint::new(xs[index % xs.len()], ys[index / xs.len()]);
    let first = index(start);
    let last = index(end);
    let mut distances = vec![f32::INFINITY; xs.len() * ys.len()];
    let mut previous = vec![usize::MAX; distances.len()];
    let mut queue = BinaryHeap::new();
    distances[first] = 0.0;
    queue.push(Visit {
        estimate: distance(start, end),
        distance: 0.0,
        node: first,
    });
    while let Some(current) = queue.pop() {
        if current.distance > distances[current.node] {
            continue;
        }
        if current.node == last {
            let mut path = vec![end];
            let mut node = last;
            while node != first {
                node = previous[node];
                path.push(point(node));
            }
            path.reverse();
            return Some(simplify(path));
        }
        let x = current.node % xs.len();
        let y = current.node / xs.len();
        let neighbors = [
            x.checked_sub(1).map(|nx| nx + y * xs.len()),
            (x + 1 < xs.len()).then_some(current.node + 1),
            y.checked_sub(1).map(|ny| x + ny * xs.len()),
            (y + 1 < ys.len()).then_some(current.node + xs.len()),
        ];
        for next in neighbors.into_iter().flatten() {
            let a = point(current.node);
            let b = point(next);
            let next_distance = current.distance + distance(a, b);
            if next_distance >= distances[next] || !clear(&[a, b], obstacles) {
                continue;
            }
            distances[next] = next_distance;
            previous[next] = current.node;
            queue.push(Visit {
                estimate: next_distance + distance(b, end),
                distance: next_distance,
                node: next,
            });
        }
    }
    None
}
fn simplify(points: Vec<CanvasPoint>) -> Vec<CanvasPoint> {
    let mut result: Vec<CanvasPoint> = Vec::new();
    for p in points {
        if result.last().is_some_and(|last| distance(*last, p) < 0.01) {
            continue;
        }
        while result.len() >= 2 {
            let a = result[result.len() - 2];
            let b = result[result.len() - 1];
            // Preserve reversals needed to depart from a boundary attachment.
            let vertical = (a.x - b.x).abs() < 0.01
                && (b.x - p.x).abs() < 0.01
                && (b.y - a.y) * (p.y - b.y) >= 0.0;
            let horizontal = (a.y - b.y).abs() < 0.01
                && (b.y - p.y).abs() < 0.01
                && (b.x - a.x) * (p.x - b.x) >= 0.0;
            if !vertical && !horizontal {
                break;
            }
            result.pop();
        }
        result.push(p);
    }
    result
}

#[cfg(test)]
#[path = "canvas_routes_tests.rs"]
mod tests;
