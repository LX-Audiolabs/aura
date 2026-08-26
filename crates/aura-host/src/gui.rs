//! Phase 2 shell: Slint window over the same host as the CLI.
//!
//! Everything here runs on the main thread — CLAP's main-thread class. Audio
//! runs in the cpal callback and is reached only through the lock-free queues.

#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
// Param ids and Slint model indices are small positive ints in practice —
// wrap/sign-loss on a u32<->i32 param id or a ComboBox row index isn't a real risk.
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::too_many_lines)]

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use clap_sys::plugin::clap_plugin;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};

use crate::audio::{self, Session};
use crate::events::{Queue, RawMidi, UiEvent};
use crate::loader::{self, PluginPtr};
use crate::midi;
use crate::plugin_gui::{self, FloatingGui};
#[cfg(windows)]
use crate::win32_embed;

slint::include_modules!();

/// Either kind of plugin window we can have open. Embedding (Phase 3, Windows
/// only) is preferred when the plugin supports it — AURA's own plugins only
/// support embedding, not floating (see `plugin_gui.rs`). Neither variant's
/// payload is read again after construction — closing/cleanup happens
/// entirely through `Drop`, which is why both look unused to the compiler.
#[allow(dead_code)]
enum PluginWindow {
    Floating(FloatingGui),
    #[cfg(windows)]
    Embedded(win32_embed::EmbeddedGui),
}

/// Raw HWND of our own top-level window, to embed a plugin's GUI into.
#[cfg(windows)]
fn parent_hwnd(ui: &HostWindow) -> Option<windows_sys::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let slint_handle = ui.window().window_handle();
    let handle = HasWindowHandle::window_handle(&slint_handle).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as windows_sys::Win32::Foundation::HWND),
        _ => None,
    }
}

/// How often the UI re-reads param values and runs the plugin's main-thread work.
const POLL: Duration = Duration::from_millis(50);

/// Where the embedded plugin socket sits in our window's client area (physical
/// px). Below the params panel; the window grows to make room for it.
#[cfg(windows)]
const EMBED_X: i32 = 16;
#[cfg(windows)]
const EMBED_Y: i32 = 480;

/// Everything the callbacks mutate. Single-threaded, hence `RefCell`.
struct Host {
    plugin: PluginPtr,
    midi_q: Queue<RawMidi>,
    ui_q: Queue<UiEvent>,
    session: Option<Session>,
    gui: Option<PluginWindow>,
    midi_conn: Option<midir::MidiInputConnection<()>>,
    /// `params()` is asked once; only values are polled after that.
    params: Vec<loader::ParamInfo>,
}

impl Host {
    fn plugin(&self) -> *const clap_plugin {
        self.plugin.0
    }
}

fn param_rows(host: &Host) -> Vec<ParamRow> {
    host.params
        .iter()
        .map(|p| {
            let value = loader::param_value(host.plugin(), p.id).unwrap_or(p.value);
            ParamRow {
                id: p.id as i32,
                name: SharedString::from(&p.name),
                value: value as f32,
                minimum: p.min as f32,
                maximum: p.max as f32,
                text: SharedString::from(loader::param_text(host.plugin(), p.id, value)),
            }
        })
        .collect()
}

/// Open the Slint shell. Returns when the window closes.
pub fn run(
    plugin: *const clap_plugin,
    name: &str,
    id: &str,
    midi_in: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ui = HostWindow::new()?;

    let host = Rc::new(RefCell::new(Host {
        plugin: PluginPtr(plugin),
        midi_q: crate::events::queue(),
        ui_q: crate::events::queue(),
        session: None,
        gui: None,
        midi_conn: None,
        params: loader::params(plugin),
    }));

    ui.set_plugin_name(SharedString::from(name));
    ui.set_plugin_id(SharedString::from(id));
    #[cfg(windows)]
    let has_gui = plugin_gui::supports_floating(plugin) || win32_embed::supports_embedded(plugin);
    #[cfg(not(windows))]
    let has_gui = plugin_gui::supports_floating(plugin);
    ui.set_gui_available(has_gui);

    let devices = audio::output_devices();
    let ports = midi::port_names();
    ui.set_audio_devices(ModelRc::new(VecModel::from(
        devices.iter().map(SharedString::from).collect::<Vec<_>>(),
    )));
    ui.set_midi_ports(ModelRc::new(VecModel::from(
        std::iter::once(SharedString::from("(none)"))
            .chain(ports.iter().map(SharedString::from))
            .collect::<Vec<_>>(),
    )));

    let params_model = Rc::new(VecModel::from(param_rows(&host.borrow())));
    ui.set_params(ModelRc::from(Rc::clone(&params_model)));

    // Start on the default device, and on the requested MIDI port if given.
    start_audio(&ui, &host, None);
    if let Some(want) = midi_in {
        let idx = ports
            .iter()
            .position(|p| p.to_lowercase().contains(&want.to_lowercase()));
        match idx {
            Some(i) => {
                ui.set_midi_index(i as i32 + 1);
                start_midi(&ui, &host, Some(&ports[i]));
            }
            None => ui.set_midi_status(SharedString::from(format!("no port matching {want:?}"))),
        }
    }

    {
        let host = Rc::clone(&host);
        ui.on_param_changed(move |id, value| {
            let h = host.borrow();
            let id = id as u32;
            let value = f64::from(value);
            // While a stream runs the audio thread applies it; otherwise flush directly.
            if h.session.is_some() {
                let _ = h.ui_q.push(UiEvent::Param { id, value });
            } else if let Err(e) = loader::set_param(h.plugin(), id, value) {
                eprintln!("warn: set param {id}: {e}");
            }
        });
    }
    {
        let host = Rc::clone(&host);
        ui.on_note_on(move |key| push_note(&host.borrow(), 0x90, key));
    }
    {
        let host = Rc::clone(&host);
        ui.on_note_off(move |key| push_note(&host.borrow(), 0x80, key));
    }
    {
        let (host, ui_w) = (Rc::clone(&host), ui.as_weak());
        ui.on_audio_device_changed(move |index| {
            let Some(ui) = ui_w.upgrade() else { return };
            let name = ui.get_audio_devices().row_data(index as usize);
            start_audio(&ui, &host, name.as_deref());
        });
    }
    {
        let (host, ui_w) = (Rc::clone(&host), ui.as_weak());
        ui.on_midi_port_changed(move |index| {
            let Some(ui) = ui_w.upgrade() else { return };
            // Row 0 is "(none)".
            let name = if index <= 0 {
                None
            } else {
                ui.get_midi_ports().row_data(index as usize)
            };
            start_midi(&ui, &host, name.as_deref());
        });
    }
    {
        let (host, ui_w) = (Rc::clone(&host), ui.as_weak());
        ui.on_toggle_gui(move || {
            let Some(ui) = ui_w.upgrade() else { return };
            let mut h = host.borrow_mut();
            if h.gui.take().is_some() {
                ui.set_gui_open(false);
                return;
            }
            let plugin = h.plugin();

            #[cfg(windows)]
            if win32_embed::supports_embedded(plugin)
                && let Some(parent) = parent_hwnd(&ui)
            {
                match win32_embed::EmbeddedGui::open(plugin, parent, EMBED_X, EMBED_Y) {
                    Ok(embedded) => {
                        let (w, h_px) = embedded.size();
                        let cur = ui.window().size();
                        ui.window().set_size(slint::WindowSize::Physical(
                            slint::PhysicalSize::new(
                                cur.width.max(EMBED_X as u32 + w + 16),
                                cur.height.max(EMBED_Y as u32 + h_px + 16),
                            ),
                        ));
                        h.gui = Some(PluginWindow::Embedded(embedded));
                        ui.set_gui_open(true);
                        return;
                    }
                    Err(e) => eprintln!("warn: embedded plugin GUI failed, trying floating: {e}"),
                }
            }

            match FloatingGui::open(plugin, ui.get_plugin_name().as_str()) {
                Ok(gui) => {
                    h.gui = Some(PluginWindow::Floating(gui));
                    ui.set_gui_open(true);
                }
                Err(e) => ui.set_log_text(SharedString::from(format!("plugin GUI: {e}"))),
            }
        });
    }

    // One timer drives everything the plugin expects from the main thread.
    let timer = slint::Timer::default();
    {
        let (host, ui_w) = (Rc::clone(&host), ui.as_weak());
        let params_model = Rc::clone(&params_model);
        timer.start(slint::TimerMode::Repeated, POLL, move || {
            let Some(ui) = ui_w.upgrade() else { return };
            loader::pump_main_thread(host.borrow().plugin());

            if loader::take_gui_closed() {
                host.borrow_mut().gui = None;
                ui.set_gui_open(false);
            }
            if loader::take_restart_request() {
                let name = ui
                    .get_audio_devices()
                    .row_data(ui.get_audio_index().max(0) as usize);
                start_audio(&ui, &host, name.as_deref());
            }

            // ponytail: polling get_value instead of reading the plugin's output
            // events — 20 Hz is enough for sliders, and it needs no return queue.
            let rows = param_rows(&host.borrow());
            for (i, row) in rows.into_iter().enumerate() {
                if params_model.row_data(i).as_ref() != Some(&row) {
                    params_model.set_row_data(i, row);
                }
            }
        });
    }

    ui.run()?;
    // Drop order: plugin window, then stream+activation, then the MIDI port.
    let mut h = host.borrow_mut();
    h.gui = None;
    h.session = None;
    h.midi_conn = None;
    Ok(())
}

fn push_note(host: &Host, status: u8, key: i32) {
    let Ok(key) = u8::try_from(key) else { return };
    if key > 127 {
        return;
    }
    let velocity = if status == 0x90 { 100 } else { 0 };
    let _ = host.ui_q.push(UiEvent::Midi([status, key, velocity]));
}

fn start_audio(ui: &HostWindow, host: &Rc<RefCell<Host>>, device: Option<&str>) {
    let mut h = host.borrow_mut();
    // Drop the old session first: it deactivates the plugin, which must happen
    // before activate() runs again.
    h.session = None;
    let (plugin, midi_q, ui_q) = (h.plugin(), Queue::clone(&h.midi_q), Queue::clone(&h.ui_q));
    match audio::open(plugin, device, midi_q, ui_q) {
        Ok(s) => {
            ui.set_audio_status(SharedString::from(format!(
                "{} Hz · {} ch · ports in {:?} / out {:?} · notes {:?}",
                s.sample_rate, s.device_channels, s.in_ports, s.out_ports, s.dialect
            )));
            h.session = Some(s);
        }
        Err(e) => ui.set_audio_status(SharedString::from(format!("stopped — {e}"))),
    }
}

fn start_midi(ui: &HostWindow, host: &Rc<RefCell<Host>>, port: Option<&str>) {
    let mut h = host.borrow_mut();
    h.midi_conn = None;
    let Some(port) = port else {
        ui.set_midi_status(SharedString::from("no MIDI input"));
        return;
    };
    let q = Queue::clone(&h.midi_q);
    match midi::open(Some(port), &q) {
        Ok(conn) => {
            h.midi_conn = Some(conn);
            ui.set_midi_status(SharedString::from(format!("listening on {port}")));
        }
        Err(e) => ui.set_midi_status(SharedString::from(e)),
    }
}
