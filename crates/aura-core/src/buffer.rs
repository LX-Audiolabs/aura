//! Non-interleaved audio buffer (minimal).

use aura_params::sample::Sample;

/// Borrowed host channel buffers for one process block.
///
/// Channel ordering is `[main inputs] [sidechain inputs] [main outputs]`.
pub struct AudioBuffer<'a, S: Sample = f32> {
    inputs: &'a [&'a [S]],
    outputs: &'a mut [&'a mut [S]],
    num_samples: usize,
    main_input_count: usize,
    sidechain_input_count: usize,
}

impl<'a, S: Sample> AudioBuffer<'a, S> {
    /// # Safety
    /// Slices must be valid for `'a`; `num_samples` must not exceed any channel length.
    /// Treats all `inputs` as main inputs (no sidechain).
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
            main_input_count: inputs.len(),
            sidechain_input_count: 0,
        }
    }

    /// Checked variant of [`from_slices`](Self::from_slices).
    ///
    /// # Panics
    /// Panics if any channel has fewer than `num_samples` elements.
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

    /// Build a buffer with a separated main + sidechain input section.
    ///
    /// `inputs` must be ordered `[main…] [sidechain…]`; `outputs` are the main outputs.
    ///
    /// # Panics
    /// Panics if `main_input_count + sidechain_input_count != inputs.len()` or if any
    /// channel is shorter than `num_samples`.
    #[must_use]
    pub fn from_slices_checked_with_sidechain(
        inputs: &'a [&'a [S]],
        outputs: &'a mut [&'a mut [S]],
        num_samples: usize,
        main_input_count: usize,
        sidechain_input_count: usize,
    ) -> Self {
        assert_eq!(main_input_count + sidechain_input_count, inputs.len());
        assert!(inputs.iter().all(|c| c.len() >= num_samples));
        assert!(outputs.iter().all(|c| c.len() >= num_samples));
        unsafe {
            Self::from_slices_with_sidechain_unchecked(
                inputs,
                outputs,
                num_samples,
                main_input_count,
                sidechain_input_count,
            )
        }
    }

    /// # Safety
    /// Same as [`from_slices`](Self::from_slices); caller must ensure counts match `inputs`.
    #[must_use]
    pub unsafe fn from_slices_with_sidechain_unchecked(
        inputs: &'a [&'a [S]],
        outputs: &'a mut [&'a mut [S]],
        num_samples: usize,
        main_input_count: usize,
        sidechain_input_count: usize,
    ) -> Self {
        debug_assert!(inputs.iter().all(|c| c.len() >= num_samples));
        debug_assert!(outputs.iter().all(|c| c.len() >= num_samples));
        Self {
            inputs,
            outputs,
            num_samples,
            main_input_count,
            sidechain_input_count,
        }
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
    pub fn num_main_inputs(&self) -> usize {
        self.main_input_count
    }

    #[must_use]
    pub fn num_sidechain_inputs(&self) -> usize {
        self.sidechain_input_count
    }

    #[must_use]
    pub fn num_outputs(&self) -> usize {
        self.outputs.len()
    }

    #[must_use]
    pub fn input(&self, ch: usize) -> &[S] {
        &self.inputs[ch][..self.num_samples]
    }

    #[must_use]
    pub fn main_input(&self, ch: usize) -> &[S] {
        &self.inputs[ch][..self.num_samples]
    }

    #[must_use]
    pub fn sidechain_input(&self, ch: usize) -> &[S] {
        &self.inputs[self.main_input_count + ch][..self.num_samples]
    }

    pub fn output(&mut self, ch: usize) -> &mut [S] {
        &mut self.outputs[ch][..self.num_samples]
    }
}
