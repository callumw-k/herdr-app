//! Arrangements: a tab laid out from its panes in order, evenly, instead of by
//! hand. Rebuilding from the order (not the old shape) makes every arrangement
//! reversible: a full cycle brings back the layout it started from.

use crate::layout::{Axis, Node};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Arrangement {
    /// Even columns, side by side.
    Vertical,
    /// Even rows, one above the other.
    Horizontal,
    /// A balanced grid of columns.
    Grid,
    /// The focused pane at full size; the others fold to title bars.
    Stacked,
}

impl Arrangement {
    pub const ALL: [Arrangement; 4] = [Arrangement::Vertical, Arrangement::Horizontal, Arrangement::Grid, Arrangement::Stacked];

    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|a| *a == self).unwrap();
        Self::ALL[(i + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Self {
        let i = Self::ALL.iter().position(|a| *a == self).unwrap();
        Self::ALL[(i + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub fn name(self) -> &'static str {
        match self {
            Arrangement::Vertical => "vertical",
            Arrangement::Horizontal => "horizontal",
            Arrangement::Grid => "grid",
            Arrangement::Stacked => "stacked",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.name() == s)
    }
}

/// What a tab with `panes` (in order) looks like under `kind`, as a tree whose
/// root keeps `root_id`. `aspect` (width / height of the tab's area) picks the
/// grid's columns. Stacked panes stay a column, so a client that doesn't know
/// arrangements still shows every pane.
pub fn arrange(kind: Arrangement, root_id: &str, panes: Vec<Node>, aspect: f32) -> Node {
    let panes: Vec<Node> = panes
        .into_iter()
        .map(|mut p| {
            p.size = None;
            p
        })
        .collect();
    match kind {
        Arrangement::Vertical => Node::container(root_id, Axis::Row, panes),
        Arrangement::Horizontal | Arrangement::Stacked => Node::container(root_id, Axis::Column, panes),
        Arrangement::Grid => grid(root_id, panes, aspect),
    }
}

fn grid(root_id: &str, panes: Vec<Node>, aspect: f32) -> Node {
    let cols = grid_columns(panes.len(), aspect);
    if cols <= 1 {
        return Node::container(root_id, Axis::Column, panes);
    }
    let (base, extra) = (panes.len() / cols, panes.len() % cols);
    let mut rest = panes.into_iter();
    let columns = (0..cols)
        .map(|i| {
            let take = base + usize::from(i < extra);
            let members: Vec<Node> = rest.by_ref().take(take).collect();
            if members.len() == 1 {
                members.into_iter().next().unwrap()
            } else {
                Node::container(format!("{root_id}-col{i}"), Axis::Column, members)
            }
        })
        .collect();
    Node::container(root_id, Axis::Row, columns)
}

/// How many columns make the squarest cells for `count` panes over an area
/// `aspect` wide per unit high, preferring full grids over ragged ones and
/// roughly equal rows and columns over a row of strips.
pub fn grid_columns(count: usize, aspect: f32) -> usize {
    let count = count.max(1);
    let aspect = if aspect.is_finite() && aspect > 0.0 { aspect } else { 1.6 };
    (1..=count)
        .min_by_key(|&cols| {
            let rows = count.div_ceil(cols);
            let cell = (aspect / cols as f32) / (1.0 / rows as f32);
            let squareness = cell.ln().abs();
            let empty = (rows * cols - count) as f32;
            let imbalance = (cols as f32 / rows as f32).ln().abs();
            ((squareness + empty * 0.5 + imbalance * 0.5) * 1000.0) as i32
        })
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Content;

    fn panes(n: usize) -> Vec<Node> {
        (0..n).map(|i| Node::block(format!("p{i}"), Content::Terminal { terminal: format!("t{i}") })).collect()
    }

    fn order(n: &Node) -> Vec<String> {
        n.panes().iter().map(|p| p.id.clone()).collect()
    }

    #[test]
    fn cycling_visits_every_arrangement_and_comes_back() {
        let mut a = Arrangement::Vertical;
        let mut seen = vec![];
        for _ in 0..4 {
            seen.push(a);
            a = a.next();
        }
        assert_eq!(a, Arrangement::Vertical);
        assert_eq!(seen, Arrangement::ALL);
        for a in Arrangement::ALL {
            assert_eq!(a.next().prev(), a);
            assert_eq!(Arrangement::parse(a.name()), Some(a));
        }
    }

    #[test]
    fn every_arrangement_keeps_the_root_id_the_pane_order_and_drops_hand_sizes() {
        let mut ps = panes(5);
        ps[1].size = Some(crate::layout::Size::Weight { weight: 0.2 });
        for a in Arrangement::ALL {
            let n = arrange(a, "tab", ps.clone(), 1.6);
            assert_eq!(n.id, "tab");
            assert_eq!(order(&n), ["p0", "p1", "p2", "p3", "p4"]);
            assert!(n.panes().iter().all(|p| p.size.is_none()), "{a:?}");
        }
    }

    #[test]
    fn vertical_is_columns_and_horizontal_and_stacked_are_rows() {
        assert_eq!(arrange(Arrangement::Vertical, "t", panes(3), 1.6).axis(), Some(Axis::Row));
        assert_eq!(arrange(Arrangement::Horizontal, "t", panes(3), 1.6).axis(), Some(Axis::Column));
        assert_eq!(arrange(Arrangement::Stacked, "t", panes(3), 1.6).axis(), Some(Axis::Column));
    }

    #[test]
    fn grids_fill_columns_evenly_with_the_extra_panes_first() {
        let n = arrange(Arrangement::Grid, "t", panes(4), 1.6);
        assert_eq!(n.axis(), Some(Axis::Row));
        assert_eq!(n.children().iter().map(|c| c.panes().len()).collect::<Vec<_>>(), [2, 2]);
        let n = arrange(Arrangement::Grid, "t", panes(5), 1.6);
        let counts: Vec<usize> = n.children().iter().map(|c| c.panes().len()).collect();
        assert_eq!(counts.iter().sum::<usize>(), 5);
        assert!(counts.windows(2).all(|w| w[0] >= w[1]), "{counts:?}");
        let ids: Vec<&str> = n.walk().iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids.len(), ids.iter().collect::<std::collections::HashSet<_>>().len());
    }

    #[test]
    fn grid_columns_follow_the_area_shape() {
        assert_eq!(grid_columns(1, 1.6), 1);
        assert_eq!(grid_columns(2, 1.6), 2);
        assert_eq!(grid_columns(2, 0.5), 1);
        assert_eq!(grid_columns(4, 1.6), 2);
        assert_eq!(grid_columns(6, 1.6), 3);
        assert_eq!(grid_columns(3, f32::NAN), grid_columns(3, 1.6));
        assert!(grid_columns(9, 2.4) >= 3);
    }

    #[test]
    fn a_lone_pane_is_one_column_in_every_arrangement() {
        for a in Arrangement::ALL {
            let n = arrange(a, "t", panes(1), 1.6);
            assert_eq!(n.children().len(), 1, "{a:?}");
        }
    }
}
