//! Fixed-capacity modulation route table.
//!
//! A preallocated, RT-safe routing table keyed by `u16` source/destination
//! ids — no closed enum surface, no heap, no per-sample allocation. Products
//! own the name → id maps. See also the legacy [`crate::mod_matrix`].

use serde::{Deserialize, Serialize};

/// A single modulation route: `src` → `dst` with a signed `depth`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModRoute {
    /// Source id (product-defined).
    pub src: u16,
    /// Destination id (product-defined).
    pub dst: u16,
    /// Modulation depth in `[-1.0, 1.0]`.
    pub depth: f32,
    /// Whether this route contributes.
    pub enabled: bool,
}

impl ModRoute {
    /// Create an enabled route; `depth` is clamped to `[-1.0, 1.0]`.
    #[must_use]
    pub fn new(src: u16, dst: u16, depth: f32) -> Self {
        Self {
            src,
            dst,
            depth: depth.clamp(-1.0, 1.0),
            enabled: true,
        }
    }
}

/// Fixed-capacity table of [`ModRoute`]s. `N` preallocated slots; `add` fails
/// (`false`) when full. No heap allocation after construction.
#[derive(Debug, Clone)]
pub struct ModRouteTable<const N: usize> {
    routes: [ModRoute; N],
    len: usize,
}

impl<const N: usize> ModRouteTable<N> {
    /// Create an empty route table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            routes: [ModRoute {
                src: 0,
                dst: 0,
                depth: 0.0,
                enabled: false,
            }; N],
            len: 0,
        }
    }

    /// Append a route. Returns `false` (no-op) if the table is full.
    pub fn add(&mut self, route: ModRoute) -> bool {
        if self.len >= N {
            return false;
        }
        self.routes[self.len] = route;
        self.len += 1;
        true
    }

    /// Remove all routes.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Number of routes currently stored.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Whether no routes are stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Slice of the currently stored routes.
    #[must_use]
    pub fn routes(&self) -> &[ModRoute] {
        &self.routes[..self.len]
    }

    /// Mutable reference to a route by index, or `None` if out of range.
    pub fn route_mut(&mut self, index: usize) -> Option<&mut ModRoute> {
        self.routes[..self.len].get_mut(index)
    }

    /// Zero `out`, then for each enabled route with in-range `src`/`dst`:
    /// `out[dst] += src_values[src] * depth`.
    ///
    /// Disabled routes and out-of-range ids are skipped (never panic).
    pub fn accumulate(&self, src_values: &[f32], out: &mut [f32]) {
        for o in out.iter_mut() {
            *o = 0.0;
        }
        for route in &self.routes[..self.len] {
            if !route.enabled {
                continue;
            }
            let src = route.src as usize;
            let dst = route.dst as usize;
            if src >= src_values.len() || dst >= out.len() {
                continue;
            }
            out[dst] += src_values[src] * route.depth;
        }
    }
}

impl<const N: usize> Default for ModRouteTable<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Serialize for ModRouteTable<N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_seq(self.routes())
    }
}

impl<'de, const N: usize> Deserialize<'de> for ModRouteTable<N> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let routes = Vec::<ModRoute>::deserialize(deserializer)?;
        if routes.len() > N {
            return Err(serde::de::Error::invalid_length(
                routes.len(),
                &"at most N routes",
            ));
        }
        let len = routes.len();
        let mut table = Self::new();
        for (slot, route) in table.routes.iter_mut().zip(routes) {
            *slot = route;
        }
        table.len = len;
        Ok(table)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulate_sums_depths_skips_disabled() {
        let mut t = ModRouteTable::<8>::new();
        assert!(t.add(ModRoute::new(0, 1, 0.5)));
        let mut disabled = ModRoute::new(0, 1, 1.0);
        disabled.enabled = false;
        assert!(t.add(disabled));
        assert!(t.add(ModRoute::new(2, 1, 0.25)));
        let src = [0.8_f32, 0.0, 0.4];
        let mut out = [0.0_f32; 4];
        t.accumulate(&src, &mut out);
        assert!((out[1] - 0.5).abs() < 1e-6, "got {}", out[1]); // 0.8*0.5 + 0.4*0.25
        assert_eq!(out[0], 0.0);
    }

    #[test]
    fn add_fails_when_full() {
        let mut t = ModRouteTable::<2>::new();
        assert!(t.add(ModRoute::new(0, 0, 0.1)));
        assert!(t.add(ModRoute::new(1, 1, 0.1)));
        assert!(!t.add(ModRoute::new(2, 2, 0.1)));
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn oob_src_dst_skipped() {
        let mut t = ModRouteTable::<4>::new();
        assert!(t.add(ModRoute::new(9, 0, 1.0)));
        assert!(t.add(ModRoute::new(0, 9, 1.0)));
        let src = [1.0_f32];
        let mut out = [0.0_f32; 1];
        t.accumulate(&src, &mut out);
        assert_eq!(out[0], 0.0);
    }

    #[test]
    fn depth_clamped_on_new() {
        assert!((ModRoute::new(0, 0, 2.0).depth - 1.0).abs() < f32::EPSILON);
        assert!((ModRoute::new(0, 0, -2.0).depth - (-1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn accumulate_clears_previous_out() {
        let mut t = ModRouteTable::<4>::new();
        assert!(t.add(ModRoute::new(0, 0, 1.0)));
        let src = [0.5_f32];
        let mut out = [9.0_f32, 9.0];
        t.accumulate(&src, &mut out);
        assert!((out[0] - 0.5).abs() < 1e-6);
        assert_eq!(out[1], 0.0);
    }
}
