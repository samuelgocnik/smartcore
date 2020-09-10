//! # Cover Tree
//!
//! The Cover Tree data structure is specifically designed to facilitate the speed-up of a nearest neighbor search, see [KNN algorithms](../index.html).
//!
//! ```
//! use smartcore::algorithm::neighbour::cover_tree::*;
//! use smartcore::math::distance::Distance;
//!
//! struct SimpleDistance {} // Our distance function
//!
//! impl Distance<i32, f64> for SimpleDistance {
//!   fn distance(&self, a: &i32, b: &i32) -> f64 { // simple simmetrical scalar distance
//!     (a - b).abs() as f64
//!   }
//! }
//!
//! let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9]; // data points
//!
//! let mut tree = CoverTree::new(data, SimpleDistance {});
//!
//! tree.find(&5, 3); // find 3 knn points from 5
//!
//! ```
use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::algorithm::sort::heap_select::HeapSelection;
use crate::math::distance::Distance;
use crate::math::num::RealNumber;

/// Implements Cover Tree algorithm
#[derive(Serialize, Deserialize, Debug)]
pub struct CoverTree<T, F: RealNumber, D: Distance<T, F>> {
    base: F,
    inv_log_base: F,
    distance: D,
    root: Node<F>,
    data: Vec<T>,
    identical_excluded: bool,
}

impl<T, F: RealNumber, D: Distance<T, F>> PartialEq for CoverTree<T, F, D> {
    fn eq(&self, other: &Self) -> bool {
        if self.data.len() != other.data.len() {
            return false;
        }
        for i in 0..self.data.len() {
            if self.distance.distance(&self.data[i], &other.data[i]) != F::zero() {
                return false;
            }
        }
        return true;
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Node<F: RealNumber> {
    idx: usize,
    max_dist: F,
    parent_dist: F,
    children: Vec<Node<F>>,
    scale: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct DistanceSet<F: RealNumber> {
    idx: usize,
    dist: Vec<F>,
}

impl<T: Debug + PartialEq, F: RealNumber, D: Distance<T, F>> CoverTree<T, F, D> {
    /// Construct a cover tree.
    /// * `data` - vector of data points to search for.
    /// * `distance` - distance metric to use for searching. This function should extend [`Distance`](../../../math/distance/index.html) interface.
    pub fn new(data: Vec<T>, distance: D) -> CoverTree<T, F, D> {
        let base = F::from_f64(1.3).unwrap();
        let root = Node {
            idx: 0,
            max_dist: F::zero(),
            parent_dist: F::zero(),
            children: Vec::new(),
            scale: 0,
        };
        let mut tree = CoverTree {
            base: base,
            inv_log_base: F::one() / base.ln(),
            distance: distance,
            root: root,
            data: data,
            identical_excluded: false,
        };

        tree.build_cover_tree();

        tree
    }

    /// Find k nearest neighbors of `p`
    /// * `p` - look for k nearest points to `p`
    /// * `k` - the number of nearest neighbors to return
    pub fn find(&self, p: &T, k: usize) -> Vec<(usize, F)> {
        if k <= 0 {
            panic!("k should be > 0");
        }

        if k > self.data.len() {
            panic!("k is > than the dataset size");
        }

        let e = self.get_data_value(self.root.idx);
        let mut d = self.distance.distance(&e, p);

        let mut current_cover_set: Vec<(F, &Node<F>)> = Vec::new();
        let mut zero_set: Vec<(F, &Node<F>)> = Vec::new();

        current_cover_set.push((d, &self.root));

        let mut heap = HeapSelection::with_capacity(k);
        heap.add(F::max_value());

        let mut empty_heap = true;
        if !self.identical_excluded || self.get_data_value(self.root.idx) != p {
            heap.add(d);
            empty_heap = false;
        }

        while !current_cover_set.is_empty() {
            let mut next_cover_set: Vec<(F, &Node<F>)> = Vec::new();
            for par in current_cover_set {
                let parent = par.1;
                for c in 0..parent.children.len() {
                    let child = &parent.children[c];
                    if c == 0 {
                        d = par.0;
                    } else {
                        d = self.distance.distance(self.get_data_value(child.idx), p);
                    }

                    let upper_bound = if empty_heap {
                        F::infinity()
                    } else {
                        *heap.peek()
                    };
                    if d <= (upper_bound + child.max_dist) {
                        if c > 0 && d < upper_bound {
                            if !self.identical_excluded || self.get_data_value(child.idx) != p {
                                heap.add(d);
                            }
                        }

                        if !child.children.is_empty() {
                            next_cover_set.push((d, child));
                        } else if d <= upper_bound {
                            zero_set.push((d, child));
                        }
                    }
                }
            }
            current_cover_set = next_cover_set;
        }

        let mut neighbors: Vec<(usize, F)> = Vec::new();
        let upper_bound = *heap.peek();
        for ds in zero_set {
            if ds.0 <= upper_bound {
                let v = self.get_data_value(ds.1.idx);
                if !self.identical_excluded || v != p {
                    neighbors.push((ds.1.idx, ds.0));
                }
            }
        }

        neighbors.into_iter().take(k).collect()
    }

    fn new_leaf(&self, idx: usize) -> Node<F> {
        Node {
            idx: idx,
            max_dist: F::zero(),
            parent_dist: F::zero(),
            children: Vec::new(),
            scale: 100,
        }
    }

    fn build_cover_tree(&mut self) {
        let mut point_set: Vec<DistanceSet<F>> = Vec::new();
        let mut consumed_set: Vec<DistanceSet<F>> = Vec::new();

        let point = &self.data[0];
        let idx = 0;
        let mut max_dist = -F::one();

        for i in 1..self.data.len() {
            let dist = self.distance.distance(point, &self.data[i]);
            let set = DistanceSet {
                idx: i,
                dist: vec![dist],
            };
            point_set.push(set);
            if dist > max_dist {
                max_dist = dist;
            }
        }

        self.root = self.batch_insert(
            idx,
            self.get_scale(max_dist),
            self.get_scale(max_dist),
            &mut point_set,
            &mut consumed_set,
        );
    }

    fn batch_insert(
        &self,
        p: usize,
        max_scale: i64,
        top_scale: i64,
        point_set: &mut Vec<DistanceSet<F>>,
        consumed_set: &mut Vec<DistanceSet<F>>,
    ) -> Node<F> {
        if point_set.is_empty() {
            self.new_leaf(p)
        } else {
            let max_dist = self.max(&point_set);
            let next_scale = (max_scale - 1).min(self.get_scale(max_dist));
            if next_scale == std::i64::MIN {
                let mut children: Vec<Node<F>> = Vec::new();
                let mut leaf = self.new_leaf(p);
                children.push(leaf);
                while !point_set.is_empty() {
                    let set = point_set.remove(point_set.len() - 1);
                    leaf = self.new_leaf(set.idx);
                    children.push(leaf);
                    consumed_set.push(set);
                }
                Node {
                    idx: p,
                    max_dist: F::zero(),
                    parent_dist: F::zero(),
                    children: children,
                    scale: 100,
                }
            } else {
                let mut far: Vec<DistanceSet<F>> = Vec::new();
                self.split(point_set, &mut far, max_scale);

                let child = self.batch_insert(p, next_scale, top_scale, point_set, consumed_set);

                if point_set.is_empty() {
                    point_set.append(&mut far);
                    child
                } else {
                    let mut children: Vec<Node<F>> = Vec::new();
                    children.push(child);
                    let mut new_point_set: Vec<DistanceSet<F>> = Vec::new();
                    let mut new_consumed_set: Vec<DistanceSet<F>> = Vec::new();

                    while !point_set.is_empty() {
                        let set: DistanceSet<F> = point_set.remove(point_set.len() - 1);

                        let new_dist: F = set.dist[set.dist.len() - 1];

                        self.dist_split(
                            point_set,
                            &mut new_point_set,
                            self.get_data_value(set.idx),
                            max_scale,
                        );
                        self.dist_split(
                            &mut far,
                            &mut new_point_set,
                            self.get_data_value(set.idx),
                            max_scale,
                        );

                        let mut new_child = self.batch_insert(
                            set.idx,
                            next_scale,
                            top_scale,
                            &mut new_point_set,
                            &mut new_consumed_set,
                        );
                        new_child.parent_dist = new_dist;

                        consumed_set.push(set);
                        children.push(new_child);

                        let fmax = self.get_cover_radius(max_scale);
                        for mut set in new_point_set.drain(0..) {
                            set.dist.remove(set.dist.len() - 1);
                            if set.dist[set.dist.len() - 1] <= fmax {
                                point_set.push(set);
                            } else {
                                far.push(set);
                            }
                        }

                        for mut set in new_consumed_set.drain(0..) {
                            set.dist.remove(set.dist.len() - 1);
                            consumed_set.push(set);
                        }
                    }

                    point_set.append(&mut far);

                    Node {
                        idx: p,
                        max_dist: self.max(consumed_set),
                        parent_dist: F::zero(),
                        children: children,
                        scale: (top_scale - max_scale),
                    }
                }
            }
        }
    }

    fn split(
        &self,
        point_set: &mut Vec<DistanceSet<F>>,
        far_set: &mut Vec<DistanceSet<F>>,
        max_scale: i64,
    ) {
        let fmax = self.get_cover_radius(max_scale);
        let mut new_set: Vec<DistanceSet<F>> = Vec::new();
        for n in point_set.drain(0..) {
            if n.dist[n.dist.len() - 1] <= fmax {
                new_set.push(n);
            } else {
                far_set.push(n);
            }
        }

        point_set.append(&mut new_set);
    }

    fn dist_split(
        &self,
        point_set: &mut Vec<DistanceSet<F>>,
        new_point_set: &mut Vec<DistanceSet<F>>,
        new_point: &T,
        max_scale: i64,
    ) {
        let fmax = self.get_cover_radius(max_scale);
        let mut new_set: Vec<DistanceSet<F>> = Vec::new();
        for mut n in point_set.drain(0..) {
            let new_dist = self
                .distance
                .distance(new_point, self.get_data_value(n.idx));
            if new_dist <= fmax {
                n.dist.push(new_dist);
                new_point_set.push(n);
            } else {
                new_set.push(n);
            }
        }

        point_set.append(&mut new_set);
    }

    fn get_cover_radius(&self, s: i64) -> F {
        self.base.powf(F::from_i64(s).unwrap())
    }

    fn get_data_value(&self, idx: usize) -> &T {
        &self.data[idx]
    }

    fn get_scale(&self, d: F) -> i64 {
        if d == F::zero() {
            std::i64::MIN
        } else {
            (self.inv_log_base * d.ln()).ceil().to_i64().unwrap()
        }
    }

    fn max(&self, distance_set: &Vec<DistanceSet<F>>) -> F {
        let mut max = F::zero();
        for n in distance_set {
            if max < n.dist[n.dist.len() - 1] {
                max = n.dist[n.dist.len() - 1];
            }
        }
        return max;
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::math::distance::Distances;

    #[derive(Debug, Serialize, Deserialize)]
    struct SimpleDistance {}

    impl Distance<i32, f64> for SimpleDistance {
        fn distance(&self, a: &i32, b: &i32) -> f64 {
            (a - b).abs() as f64
        }
    }

    #[test]
    fn cover_tree_test() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];

        let tree = CoverTree::new(data, SimpleDistance {});

        let mut knn = tree.find(&5, 3);
        knn.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let knn: Vec<usize> = knn.iter().map(|v| v.0).collect();
        assert_eq!(vec!(3, 4, 5), knn);
    }

    #[test]
    fn cover_tree_test1() {
        let data = vec![
            vec![1., 2.],
            vec![3., 4.],
            vec![5., 6.],
            vec![7., 8.],
            vec![9., 10.],
        ];

        let tree = CoverTree::new(data, Distances::euclidian());

        let mut knn = tree.find(&vec![1., 2.], 3);
        knn.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let knn: Vec<usize> = knn.iter().map(|v| v.0).collect();

        assert_eq!(vec!(0, 1, 2), knn);
    }

    #[test]
    fn serde() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9];

        let tree = CoverTree::new(data, SimpleDistance {});

        let deserialized_tree: CoverTree<i32, f64, SimpleDistance> =
            serde_json::from_str(&serde_json::to_string(&tree).unwrap()).unwrap();

        assert_eq!(tree, deserialized_tree);
    }
}
