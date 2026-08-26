//! aura-host — minimal CLAP host.
//!
//! Phase 1 is the CLI (params, MIDI, audio); `--gui` opens the Phase 2 Slint
//! shell with device pick, param sliders and computer-keyboard notes.
//!
//! Usage:
//!   aura-host <path.clap> [--plugin <id>] [--gui] [--list-params]
//!                         [--set <id>=<val>] [--play] [--list-midi]
//!                         [--midi-in <name>]

#![allow(clippy::missing_safety_doc)]

mod audio;
mod events;
mod gui;
mod loader;
mod midi;
mod plugin_gui;
#[cfg(windows)]
mod win32_embed;

use std::ffi::CStr;

const USAGE: &str = "usage: aura-host <path.clap> [--plugin <id>] [--gui] [--list-params] \
                     [--set <id>=<val>] [--play] [--list-midi] [--midi-in <name>]";

// A CLI flag struct is exactly the case this lint doesn't help — each bool
// is an independent switch, not related state that wants an enum.
#[allow(clippy::struct_excessive_bools)]
struct Args {
    path: String,
    plugin_id: Option<String>,
    list_params: bool,
    play: bool,
    gui: bool,
    list_midi: bool,
    midi_in: Option<String>,
    sets: Vec<(u32, f64)>,
}

fn parse_args() -> Args {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let mut a = Args {
        path: argv[0].clone(),
        plugin_id: None,
        list_params: false,
        play: false,
        gui: false,
        list_midi: false,
        midi_in: None,
        sets: Vec::new(),
    };

    let mut i = 1;
    while i < argv.len() {
        match argv[i].as_str() {
            "--plugin" => {
                i += 1;
                a.plugin_id = argv.get(i).cloned();
            }
            "--midi-in" => {
                i += 1;
                a.midi_in = argv.get(i).cloned();
            }
            "--set" => {
                i += 1;
                if let Some(kv) = argv.get(i).and_then(|s| parse_set(s)) {
                    a.sets.push(kv);
                } else {
                    eprintln!(
                        "error: --set wants <param_id>=<value>, got {:?}",
                        argv.get(i)
                    );
                    std::process::exit(1);
                }
            }
            "--list-params" => a.list_params = true,
            "--play" => a.play = true,
            "--gui" => a.gui = true,
            "--list-midi" => a.list_midi = true,
            arg => eprintln!("warn: unknown arg {arg}"),
        }
        i += 1;
    }
    a
}

/// `"3=0.5"` → `(3, 0.5)`.
fn parse_set(s: &str) -> Option<(u32, f64)> {
    let (id, val) = s.split_once('=')?;
    Some((id.trim().parse().ok()?, val.trim().parse().ok()?))
}

/// Name and id of the plugin the GUI is showing — same pick as `Loader::create`.
fn descriptor_strings(loader: &loader::Loader, want_id: Option<&str>) -> (String, String) {
    for idx in 0..loader.plugin_count() {
        let Some(d) = loader.descriptor(idx) else {
            continue;
        };
        let id = unsafe { CStr::from_ptr(d.id) }
            .to_string_lossy()
            .into_owned();
        if want_id.is_none_or(|want| want == id) {
            let name = unsafe { CStr::from_ptr(d.name) }
                .to_string_lossy()
                .into_owned();
            return (name, id);
        }
    }
    (String::new(), String::new())
}

fn main() {
    let args = parse_args();

    if args.list_midi {
        midi::list_ports();
    }

    let loader = unsafe { loader::Loader::open(&args.path) }.unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // Always list plugins in the file.
    let n = loader.plugin_count();
    println!("found {n} plugin(s) in {}", args.path);
    for idx in 0..n {
        if let Some(d) = loader.descriptor(idx) {
            let name = unsafe { CStr::from_ptr(d.name) }.to_string_lossy();
            let id = unsafe { CStr::from_ptr(d.id) }.to_string_lossy();
            println!("  [{idx}] {name}  id={id}");
        }
    }

    if !args.list_params && !args.play && !args.gui && args.sets.is_empty() {
        return;
    }

    let host = loader::make_host();
    let plugin = loader
        .create(host, args.plugin_id.as_deref())
        .unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

    if let Some(init) = unsafe { (*plugin).init }
        && !unsafe { init(plugin) }
    {
        eprintln!("error: plugin.init returned false");
        std::process::exit(1);
    }

    // Applied while deactivated, so the values are in effect before activate().
    for (id, value) in &args.sets {
        match loader::set_param(plugin, *id, *value) {
            Ok(()) => println!("set param {id} = {value}"),
            Err(e) => eprintln!("error: set param {id}: {e}"),
        }
    }

    if args.list_params {
        loader::list_params(plugin);
    }

    if args.gui {
        let (name, id) = descriptor_strings(&loader, args.plugin_id.as_deref());
        if let Err(e) = gui::run(plugin, &name, &id, args.midi_in.as_deref()) {
            eprintln!("error: gui: {e}");
            std::process::exit(1);
        }
    } else if args.play {
        let midi_q = events::queue();
        // Connection must outlive the stream: dropping it closes the MIDI port.
        let _conn = midi::open(args.midi_in.as_deref(), &midi_q)
            .inspect_err(|e| eprintln!("warn: {e} — running without MIDI"))
            .ok();
        audio::run(plugin, midi_q, events::queue());
        // run() loops forever; if it returns, fall through to destroy.
    }

    if let Some(destroy) = unsafe { (*plugin).destroy } {
        unsafe { destroy(plugin) };
    }
}
