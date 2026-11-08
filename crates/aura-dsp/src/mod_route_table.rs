//! Fixed-capacity modulation route table.
//!
//! A preallocated, RT-safe routing table keyed by `u16` source/destination
//! ids — no closed enum surface, no heap, no per-sample allocation. Products
//! own the name → id maps. See also the legacy [`crate::mod_matrix`].

use serde::{Deserialize, Serialize};

/// A single modulation route: `src` → `dst` with a signed normalized amount
/// and a destination range scale in product-defined units.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModRoute {
    /// Source id (product-defined).
    pub src: u16,
    /// Destination id (product-defined).
    pub dst: u16,
    /// Normalized modulation amount in `[-1.0, 1.0]` when constructed.
    pub depth: f32,
    /// Destination range scale in product-defined units; defaults to `1.0`.
    #[serde(default = "default_route_scale")]
    pub scale: f32,
    /// Whether this route contributes.
    pub enabled: bool,
}

impl ModRoute {
    /// Create an enabled route; `depth` is clamped to `[-1.0, 1.0]`.
    /// Non-finite depth becomes `0.0` (NaN must not enter the table via `new`).
    #[must_use]
    pub fn new(src: u16, dst: u16, depth: f32) -> Self {
        Self::new_scaled(src, dst, depth, 1.0)
    }

    /// Create an enabled route with a destination range `scale`.
    /// `depth` is clamped to `[-1.0, 1.0]`; non-finite inputs become `0.0`.
    #[must_use]
    pub fn new_scaled(src: u16, dst: u16, depth: f32, scale: f32) -> Self {
        Self {
            src,
            dst,
            depth: if depth.is_finite() {
                depth.clamp(-1.0, 1.0)
            } else {
                0.0
            },
            scale: if scale.is_finite() { scale } else { 0.0 },
            enabled: true,
        }
    }
}

fn default_route_scale() -> f32 {
    1.0
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
                scale: 1.0,
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
    /// `out[dst] += src_values[src] * depth * scale`.
    ///
    /// Disabled routes, out-of-range ids, and non-finite `depth` / `scale` / source
    /// samples are skipped (never panic; no NaN poison into `out`).
    pub fn accumulate(&self, src_values: &[f32], out: &mut [f32]) {
        for o in out.iter_mut() {
            *o = 0.0;
        }
        for route in &self.routes[..self.len] {
            if !route.enabled || !route.depth.is_finite() || !route.scale.is_finite() {
                continue;
            }
            let src = route.src as usize;
            let dst = route.dst as usize;
            if src >= src_values.len() || dst >= out.len() {
                continue;
            }
            let s = src_values[src];
            if !s.is_finite() {
                continue;
            }
            let contribution = s * route.depth * route.scale;
            if contribution.is_finite() {
                let sum = out[dst] + contribution;
                if sum.is_finite() {
                    out[dst] = sum;
                }
            }
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
        assert_eq!(ModRoute::new(0, 0, 0.5).scale, 1.0);
        assert!((ModRoute::new(0, 0, 2.0).depth - 1.0).abs() < f32::EPSILON);
        assert!((ModRoute::new(0, 0, -2.0).depth - (-1.0)).abs() < f32::EPSILON);
        assert_eq!(ModRoute::new(0, 0, f32::NAN).depth, 0.0);
        assert_eq!(ModRoute::new(0, 0, f32::INFINITY).depth, 0.0);
    }

    #[test]
    fn scaled_route_preserves_product_units_and_normalizes_invalid_inputs() {
        let route = ModRoute::new_scaled(0, 0, 2.0, 12.0);
        assert_eq!(route.depth, 1.0);
        assert_eq!(route.scale, 12.0);
        assert!(route.enabled);
        assert_eq!(ModRoute::new_scaled(0, 0, f32::NAN, 8.0).depth, 0.0);
        assert_eq!(ModRoute::new_scaled(0, 0, 1.0, f32::NAN).scale, 0.0);
        assert_eq!(ModRoute::new_scaled(0, 0, 1.0, f32::INFINITY).scale, 0.0);
    }

    #[test]
    fn accumulate_applies_scale_and_skips_invalid_mutated_scale() {
        let mut table = ModRouteTable::<3>::new();
        assert!(table.add(ModRoute::new_scaled(0, 0, 1.0, 12.0)));
        assert!(table.add(ModRoute::new_scaled(1, 1, 0.5, 8.0)));
        assert!(table.add(ModRoute::new_scaled(0, 0, 1.0, 20.0)));
        table.route_mut(2).unwrap().scale = f32::NAN;
        let mut out = [9.0; 2];
        table.accumulate(&[1.0, 0.5], &mut out);
        assert_eq!(out, [12.0, 2.0]);
    }

    #[test]
    fn accumulate_keeps_output_finite_when_products_overflow() {
        let mut table = ModRouteTable::<3>::new();
        assert!(table.add(ModRoute::new_scaled(0, 0, 1.0, 2.0)));
        assert!(table.add(ModRoute::new(1, 0, 1.0)));
        assert!(table.add(ModRoute::new(2, 0, 1.0)));
        let mut out = [0.0];
        table.accumulate(&[f32::MAX, f32::MAX, f32::MAX], &mut out);
        assert_eq!(out, [f32::MAX]);
    }

    #[test]
    fn serde_preserves_scale_and_defaults_old_routes_to_identity() {
        let route = ModRoute::new_scaled(1, 2, 0.5, 12.0);
        let json = serde_json::to_string(&route).unwrap();
        assert_eq!(serde_json::from_str::<ModRoute>(&json).unwrap(), route);
        let old = r#"{"src":1,"dst":2,"depth":0.5,"enabled":true}"#;
        assert_eq!(serde_json::from_str::<ModRoute>(old).unwrap().scale, 1.0);
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

    #[test]
    fn accumulate_skips_non_finite_depth_and_src() {
        let mut t = ModRouteTable::<4>::new();
        assert!(t.add(ModRoute::new(0, 0, 1.0)));
        t.route_mut(0).unwrap().depth = f32::NAN;
        assert!(t.add(ModRoute::new(1, 0, 1.0)));
        let src = [0.5_f32, f32::NAN];
        let mut out = [9.0_f32];
        t.accumulate(&src, &mut out);
        assert_eq!(out[0], 0.0);
    }

    #[test]
    fn serde_roundtrip_and_too_long_err() {
        let mut t = ModRouteTable::<4>::new();
        assert!(t.add(ModRoute::new(1, 2, 0.5)));
        assert!(t.add(ModRoute::new(3, 4, -0.25)));
        let json = serde_json::to_string(&t).unwrap();
        let back: ModRouteTable<4> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.routes()[0].src, 1);
        assert_eq!(back.routes()[0].dst, 2);
        assert!((back.routes()[0].depth - 0.5).abs() < f32::EPSILON);
        assert_eq!(back.routes()[1].src, 3);
        assert!((back.routes()[1].depth - (-0.25)).abs() < f32::EPSILON);

        let too_long = vec![ModRoute::new(0, 0, 0.1); 3];
        let bad = serde_json::to_string(&too_long).unwrap();
        assert!(serde_json::from_str::<ModRouteTable<2>>(&bad).is_err());
    }
}
