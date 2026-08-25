//! aura-host — minimal CLAP host (Phase 1: CLI, params, audio)
//!
//! Usage:
//!   aura-host <path.clap> [--plugin <id>] [--list-params] [--play]

#![allow(clippy::missing_safety_doc)]

mod loader;

use std::ffi::CStr;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!(
            "usage: aura-host <path.clap> [--plugin <id>] [--list-params] [--play]"
        );
        std::process::exit(1);
    }

    let path = &args[0];
    let mut plugin_id: Option<String> = None;
    let mut list_params = false;
    let mut play = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--plugin" => {
                i += 1;
                plugin_id = args.get(i).cloned();
            }
            "--list-params" => list_params = true,
            "--play" => play = true,
            arg => eprintln!("warn: unknown arg {arg}"),
        }
        i += 1;
    }

    let loader = unsafe { loader::Loader::open(path) }.unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // Always list plugins in the file.
    let n = loader.plugin_count();
    println!("found {n} plugin(s) in {path}");
    for idx in 0..n {
        if let Some(d) = loader.descriptor(idx) {
            let name = unsafe { CStr::from_ptr(d.name) }.to_string_lossy();
            let id = unsafe { CStr::from_ptr(d.id) }.to_string_lossy();
            println!("  [{idx}] {name}  id={id}");
        }
    }

    if !list_params && !play {
        return;
    }

    let host = loader::make_host();
    let plugin = loader.create(host, plugin_id.as_deref()).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    if let Some(init) = unsafe { (*plugin).init } {
        if !unsafe { init(plugin) } {
            eprintln!("error: plugin.init returned false");
            std::process::exit(1);
        }
    }

    if list_params {
        loader::list_params(plugin);
    }

    if play {
        run_audio(plugin);
        // run_audio loops forever; if it returns, fall through to destroy.
    }

    if let Some(destroy) = unsafe { (*plugin).destroy } {
        unsafe { destroy(plugin) };
    }
}

fn run_audio(plugin: *const clap_sys::plugin::clap_plugin) {
    let audio_host = cpal::default_host();
    let device = audio_host.default_output_device().unwrap_or_else(|| {
        eprintln!("error: no default output device");
        std::process::exit(1);
    });
    let config = device.default_output_config().unwrap_or_else(|e| {
        eprintln!("error: output config: {e}");
        std::process::exit(1);
    });

    let sample_rate = config.sample_rate().0 as f64;
    let channels = config.channels() as usize;

    // Activate with a generous max_frames; actual frame count comes per-callback.
    if let Some(activate) = unsafe { (*plugin).activate } {
        if !unsafe { activate(plugin, sample_rate, 1, 4096) } {
            eprintln!("error: plugin.activate returned false");
            std::process::exit(1);
        }
    }
    if let Some(start) = unsafe { (*plugin).start_processing } {
        if !unsafe { start(plugin) } {
            eprintln!("error: plugin.start_processing returned false");
            std::process::exit(1);
        }
    }

    // PluginPtr is Send (see loader.rs); captures pp whole so Rust 2021 precision
    // captures don't reduce to the inner *const clap_plugin which is !Send.
    let pp = loader::PluginPtr(plugin);

    let stream_config = cpal::StreamConfig {
        channels: config.channels(),
        sample_rate: config.sample_rate(),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device
            .build_output_stream::<f32, _, _>(
                &stream_config,
                move |data: &mut [f32], _| {
                    loader::audio_callback(pp, data, channels);
                },
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
