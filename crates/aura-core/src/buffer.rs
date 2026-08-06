//! Non-interleaved audio buffer (minimal).

use aura_params::sample::Sample;

/// Borrowed host channel buffers for one process block.
pub struct AudioBuffer<'a, S: Sample = f32> {
    inputs: &'a [&'a [S]],
    outputs: &'a mut [&'a mut [S]],
    num_samples: usize,
}

impl<'a, S: Sample> AudioBuffer<'a, S> {
    /// # Safety
    /// Slices must be valid for `'a`; `num_samples` must not exceed any channel length.
    #[must_use]
    pub unsafe fn from_slices(
        inputs: &'a [&'a [S]],
        outputs: &'a mut [&'a mut [S]],
        num_samples: usize,
    ) -> Self {
        debug_assert!(inputs.iter().all(|c| c.len() >= num_samples));
        debug_assert!(outputs.iter().all(|c| c.len() >= num_samples));
        Self {
            inputs,
            outputs,
            num_samples,
        }
    }

    #[must_use]
    pub fn from_slices_checked(
        inputs: &'a [&'a [S]],
        outputs: &'a mut [&'a mut [S]],
        num_samples: usize,
    ) -> Self {
        // SAFETY: checked lengths below
        assert!(inputs.iter().all(|c| c.len() >= num_samples));
        assert!(outputs.iter().all(|c| c.len() >= num_samples));
        unsafe { Self::from_slices(inputs, outputs, num_samples) }
    }

    #[must_use]
    pub fn num_samples(&self) -> usize {
        self.num_samples
    }

    #[must_use]
    pub fn num_inputs(&self) -> usize {
        self.inputs.len()
    }

    #[must_use]
    pub fn num_outputs(&self) -> usize {
        self.outputs.len()
    }

    #[must_use]
    pub fn input(&self, ch: usize) -> &[S] {
        &self.inputs[ch][..self.num_samples]
    }

    pub fn output(&mut self, ch: usize) -> &mut [S] {
        &mut self.outputs[ch][..self.num_samples]
    }
}
