//! Private endpoint-aware delta TDF-II: f64 coefficients, f32 audio/state.

#[derive(Debug, Clone, Default)]
pub(crate) struct BiquadKernel {
    source: Option<[f64; 5]>,
    tick: [f32; 6],
    // z1 and z1 + sign*z2 retain coefficient-independent TDF-II history.
    first: f32,
    sum: f32,
}

impl BiquadKernel {
    fn prepare(&mut self, coefs: [f64; 5]) {
        if self.source == Some(coefs) {
            return;
        }
        let [b0, b1, b2, a1, a2] = coefs;
        let sign = if a1 > 0.0 { -1.0 } else { 1.0 };
        if self.source.is_some() && self.tick[5] != sign as f32 {
            self.sum = 2.0 * self.first - self.sum;
        }
        let p = 1.0 + sign * a1 + a2;
        let feed = b1 - a1 * b0;
        let drive = sign * ((b0 + sign * b1 + b2) - p * b0);
        self.tick = [
            b0 as f32,
            feed as f32,
            drive as f32,
            p as f32,
            (2.0 + sign * a1) as f32,
            sign as f32,
        ];
        self.source = Some(coefs);
    }

    pub(crate) fn reset(&mut self) {
        self.first = 0.0;
        self.sum = 0.0;
    }

    /// Retain the small denominator sum near either DC or Nyquist rather than
    /// rounding poles onto/outside the unit circle in a direct f32 realization.
    /// No divisions; exact cancelled-pole identity coefficients remain valid.
    #[inline]
    pub(crate) fn process_buffer(&mut self, buffer: &mut [f32], coefs: [f64; 5]) {
        if buffer.is_empty() {
            return;
        }
        self.prepare(coefs);
        if !buffer.iter().all(|x| x.is_finite()) || !self.tick.iter().all(|x| x.is_finite()) {
            buffer.fill(0.0);
            self.reset();
            return;
        }
        let [direct, feed, drive, p, c, sign] = self.tick;
        let mut first = self.first;
        let mut sum = self.sum;
        // True recurrence: deliberately scalar over time, no per-sample guards.
        for x in buffer.iter_mut() {
            let y = direct.mul_add(*x, first);
            let next = sign.mul_add(first + (-c).mul_add(first, sum), feed * *x);
            sum = sign.mul_add((-p).mul_add(first, sum), drive * *x);
            first = next;
            *x = y;
        }
        self.first = first;
        self.sum = sum;
        if !first.is_finite() || !sum.is_finite() || !buffer.iter().all(|x| x.is_finite()) {
            buffer.fill(0.0);
            self.reset();
        }
    }
}
