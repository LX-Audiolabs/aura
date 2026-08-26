//! cpal output stream → `clap_process`, with MIDI drained from the ring buffer.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

use std::ptr;

use clap_sys::{audio_buffer::clap_audio_buffer, plugin::clap_plugin, process::clap_process};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::events::{Dialect, EvList, RawMidi, sink_output_events};
use crate::loader::{self, PluginPtr};

/// Frames per `process()` call at most — also the `max_frames_count` we activate with.
pub const MAX_FRAMES: usize = 4096;

/// Owns everything the audio thread touches. Buffers are allocated once; the
/// callback never allocates.
pub struct Engine {
    plugin: PluginPtr,
    device_channels: usize,
    /// One channel buffer per (port, channel), flattened; kept alive for the
    /// pointers in `in_ports` / `out_ports`.
    bufs: Vec<Vec<f32>>,
    /// Channel pointer array the `clap_audio_buffer`s point into. Never read
    /// directly — it just has to stay alive and unmoved.
    _ptrs: Vec<*mut f32>,
    in_ports: Vec<clap_audio_buffer>,
    out_ports: Vec<clap_audio_buffer>,
    /// Channels of output port 0 — what the device actually hears.
    main_out_channels: usize,
    /// Index into `bufs` where output port 0 starts.
    main_out_offset: usize,
    events: EvList,
    midi_rx: rtrb::Consumer<RawMidi>,
    dialect: Dialect,
    steady_time: i64,
}

// Safety: the Engine is built on the main thread and then moved into the cpal
// callback, where it lives on exactly one thread. Raw pointers are into its own
// buffers (and the plugin, which outlives the stream).
unsafe impl Send for Engine {}

impl Engine {
    #[must_use]
    pub fn new(
        plugin: *const clap_plugin,
        device_channels: usize,
        midi_rx: rtrb::Consumer<RawMidi>,
    ) -> Self {
        let in_counts = loader::audio_port_channels(plugin, true);
        let mut out_counts = loader::audio_port_channels(plugin, false);
        // A plugin with no audio-ports extension still has to be heard somehow.
        if out_counts.iter().sum::<u32>() == 0 {
            out_counts = vec![2];
        }

        let main_out_offset = in_counts.iter().sum::<u32>() as usize;
        let main_out_channels = out_counts[0].max(1) as usize;

        let total_ch = main_out_offset + out_counts.iter().sum::<u32>() as usize;
        let mut bufs: Vec<Vec<f32>> = (0..total_ch).map(|_| vec![0.0; MAX_FRAMES]).collect();
        // Neither Vec ever resizes again, so these pointers stay valid for the
        // Engine's lifetime — including after it is moved into the callback.
        let mut ptrs: Vec<*mut f32> = bufs.iter_mut().map(Vec::as_mut_ptr).collect();

        let mut next = 0;
        let mut port_bufs = |counts: &[u32]| {
            counts
                .iter()
                .map(|&ch| {
                    let base = unsafe { ptrs.as_mut_ptr().add(next) };
                    next += ch as usize;
                    clap_audio_buffer {
                        data32: base,
                        data64: ptr::null_mut(),
                        channel_count: ch,
                        latency: 0,
                        constant_mask: 0,
                    }
                })
                .collect::<Vec<_>>()
        };
        let in_ports = port_bufs(&in_counts);
        let out_ports = port_bufs(&out_counts);

        Self {
            plugin: PluginPtr(plugin),
            device_channels,
            bufs,
            _ptrs: ptrs,
            in_ports,
            out_ports,
            main_out_channels,
            main_out_offset,
            events: EvList::with_capacity(256),
            midi_rx,
            dialect: loader::note_dialect(plugin),
            steady_time: 0,
        }
    }

    /// Channels per port, `(inputs, outputs)` — for the startup banner.
    #[must_use]
    pub fn port_layout(&self) -> (Vec<u32>, Vec<u32>) {
        let chans = |ports: &[clap_audio_buffer]| ports.iter().map(|p| p.channel_count).collect();
        (chans(&self.in_ports), chans(&self.out_ports))
    }

    #[must_use]
    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// cpal callback body. `data` is interleaved f32 for `device_channels`.
    pub fn process(&mut self, data: &mut [f32]) {
        loader::mark_audio_thread();
        if self.device_channels == 0 {
            return;
        }
        // ponytail: every queued MIDI message lands at frame 0 of the next block —
        // sample-accurate timestamps need cpal's OutputCallbackInfo, add if it matters.
        self.events.clear();
        while let Ok(msg) = self.midi_rx.pop() {
            self.events.push_midi(msg, self.dialect, 0);
        }

        let total = data.len() / self.device_channels;
        let mut done = 0;
        while done < total {
            let frames = (total - done).min(MAX_FRAMES);
            self.process_block(frames);
            self.interleave(data, done, frames);
            // Events belong to the first block only.
            self.events.clear();
            self.steady_time += frames as i64;
            done += frames;
        }
    }

    fn process_block(&mut self, frames: usize) {
        // ponytail: inputs are silence — this host has no capture path yet.
        for b in &mut self.bufs[..self.main_out_offset] {
            b[..frames].fill(0.0);
        }
        for p in &mut self.in_ports {
            p.constant_mask = 0;
        }

        let in_ev = self.events.as_input_events();
        let out_ev = sink_output_events();

        let proc = clap_process {
            steady_time: self.steady_time,
            frames_count: frames as u32,
            transport: ptr::null(),
            audio_inputs: if self.in_ports.is_empty() {
                ptr::null()
            } else {
                self.in_ports.as_ptr()
            },
            audio_outputs: self.out_ports.as_mut_ptr(),
            audio_inputs_count: self.in_ports.len() as u32,
            audio_outputs_count: self.out_ports.len() as u32,
            in_events: &raw const in_ev,
            out_events: &raw const out_ev,
        };

        if let Some(process_fn) = unsafe { (*self.plugin.0).process } {
            unsafe { process_fn(self.plugin.0, &raw const proc) };
        }
    }

    /// Non-interleaved plugin output → interleaved device buffer. Fewer plugin
    /// channels than device channels: the last one is repeated (mono → stereo).
    fn interleave(&self, data: &mut [f32], frame_offset: usize, frames: usize) {
        let dev = self.device_channels;
        for ch in 0..dev {
            let src = &self.bufs[self.main_out_offset + ch.min(self.main_out_channels - 1)];
            for i in 0..frames {
                data[(frame_offset + i) * dev + ch] = src[i];
            }
        }
    }
}

/// Activate the plugin, open the default output device, and block forever.
pub fn run(plugin: *const clap_plugin, midi_rx: rtrb::Consumer<RawMidi>) {
    let audio_host = cpal::default_host();
    let device = audio_host.default_output_device().unwrap_or_else(|| {
        eprintln!("error: no default output device");
        std::process::exit(1);
    });
    let config = device.default_output_config().unwrap_or_else(|e| {
        eprintln!("error: output config: {e}");
        std::process::exit(1);
    });

    let sample_rate = f64::from(config.sample_rate().0);
    let channels = config.channels() as usize;

    if let Some(activate) = unsafe { (*plugin).activate }
        && !unsafe { activate(plugin, sample_rate, 1, MAX_FRAMES as u32) }
    {
        eprintln!("error: plugin.activate returned false");
        std::process::exit(1);
    }
    if let Some(start) = unsafe { (*plugin).start_processing }
        && !unsafe { start(plugin) }
    {
        eprintln!("error: plugin.start_processing returned false");
        std::process::exit(1);
    }

    let mut engine = Engine::new(plugin, channels, midi_rx);
    let (ins, outs) = engine.port_layout();
    println!(
        "audio ports: in {ins:?} / out {outs:?} (channels per port), note dialect {:?}",
        engine.dialect()
    );

    let stream_config = cpal::StreamConfig {
        channels: config.channels(),
        sample_rate: config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_output_stream::<f32, _, _>(
                &stream_config,
                move |data: &mut [f32], _| engine.process(data),
                |e| eprintln!("audio error: {e}"),
                None,
            )
            .unwrap_or_else(|e| {
                eprintln!("error: build_output_stream: {e}");
                std::process::exit(1);
            }),
        fmt => {
            eprintln!("error: unsupported sample format {fmt:?} — add conversion if needed");
            std::process::exit(1);
        }
    };

    stream.play().unwrap_or_else(|e| {
        eprintln!("error: stream.play: {e}");
        std::process::exit(1);
    });

    println!("playing at {sample_rate} Hz, {channels}ch — ctrl+c to stop");
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
