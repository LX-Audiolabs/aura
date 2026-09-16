# CLAP Guide for AURA

*A comprehensive guide to using CLAP extensions in AURA*

## **1. Introduction to CLAP**

### Why CLAP?
CLAP (CLever Audio Plugin) is a modern, open-source plugin format designed for **modularity, performance, and extensibility**. Unlike VST3, LV2, AU, or AAX, CLAP is **not tied to a specific company** and offers **first-class support for advanced features** like:
- **Polyphonic modulation**
- **Note expressions**
- **Remote controls**
- **Tuning systems**
- **State management**

### AURA’s CLAP Advantages
AURA is a **CLAP-first framework** that provides:
- **Full support for all CLAP extensions** (no compromises).
- **Abstraction over low-level details** (e.g., `clap-sys`).
- **Seamless integration with Slint for UI**.
- **Cross-platform compatibility** (Windows, macOS, Linux).

---

## **2. Getting Started: A Minimal CLAP Plugin**

### Basic Structure
Here’s a minimal example of a CLAP plugin in AURA:

```rust
use aura_clap::Instance;
use clap_sys::plugin::clap_plugin_descriptor;
use aura_core::Plugin;

#[derive(Default)]
struct MyPlugin;

impl Plugin for MyPlugin {
    fn process(&mut self, instance: &mut Instance, process: &clap_process) -> Result<(), ()> {
        // Audio processing logic here
        Ok(())
    }
}

// CLAP descriptor
#[no_mangle]
pub static CLAP_DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: clap_sys::version::CLAP_VERSION,
    id: b"com.example.myplugin\0".as_ptr() as *const _,
    name: b"My Plugin\0".as_ptr() as *const _,
    vendor: b"Example\0".as_ptr() as *const _,
    url: b"https://example.com\0".as_ptr() as *const _,
    manual_url: b"https://example.com/manual\0".as_ptr() as *const _,
    support_url: b"https://example.com/support\0".as_ptr() as *const _,
    version: b"1.0.0\0".as_ptr() as *const _,
    description: b"A simple CLAP plugin\0".as_ptr() as *const _,
    features: &[
        clap_sys::plugin_features::CLAP_PLUGIN_FEATURE_AUDIO_EFFECT,
        std::ptr::null(),
    ][0] as *const _,
};
```

---

## **3. CLAP Extensions in AURA**

AURA supports **all CLAP extensions**. Below are the most important ones with examples.

### **3.1 Parameters (`clap.params`)**

#### Dynamic Parameters
Use `aura-params` to define and manage parameters:

```rust
use aura_params::Param;

#[derive(Params)]
struct MyParams {
    #[id = "gain"]
    gain: Param<f32>,
    #[id = "cutoff"]
    cutoff: Param<f32>,
}

impl Default for MyParams {
    fn default() -> Self {
        Self {
            gain: Param::new("Gain", 0.5, 0.0..=1.0),
            cutoff: Param::new("Cutoff", 1000.0, 20.0..=20000.0),
        }
    }
}
```

#### Parameter Events
Handle parameter changes in the audio thread:

```rust
unsafe fn emit_param_events(instance: &mut Instance, events: &[clap_event_param_value]) {
    for event in events {
        instance.param_events.push(*event);
    }
}
```

---

### **3.2 GUI (`clap.gui`)**

#### Creating a GUI
Use `aura-baseview` and `Slint` for hardware-accelerated UIs:

```rust
use aura_baseview::SlintParentedWindow;
use slint::PhysicalSize;

fn create_gui(instance: &mut Instance) {
    let window = SlintParentedWindow::new(
        PhysicalSize::new(800, 600),
        instance.host_scale(),
    );
    instance.set_editor(window);
}
```

#### Scaling for HiDPI
AURA automatically handles scaling between logical and physical pixels:

```rust
fn set_scale(instance: &mut Instance, scale: f64) {
    instance.set_host_scale(scale);
}
```

---

### **3.3 State (`clap.state`)**

#### Saving and Loading State
Serialize and deserialize plugin state:

```rust
use clap_sys::stream::{clap_istream, clap_ostream};

unsafe extern "C" fn state_save(
    plugin: *const clap_plugin,
    stream: *const clap_ostream,
) -> bool {
    let instance = &mut *(plugin as *mut Instance);
    let data = bincode::serialize(&instance.state).unwrap();
    let written = (*stream).write.unwrap()(stream as *const _, data.as_ptr() as *const _, data.len());
    written == data.len()
}

unsafe extern "C" fn state_load(
    plugin: *const clap_plugin,
    stream: *const clap_istream,
) -> bool {
    let instance = &mut *(plugin as *mut Instance);
    let mut data = vec![0u8; (*stream).size as usize];
    let read = (*stream).read.unwrap()(stream as *const _, data.as_mut_ptr() as *mut _, data.len());
    if read == data.len() {
        instance.state = bincode::deserialize(&data).unwrap();
        true
    } else {
        false
    }
}
```

---

### **3.4 Remote Controls (`clap.remote-controls`)**

#### MIDI Mapping for Hardware Controllers
Define remote control pages for hardware integration:

```rust
use clap_sys::remote_controls::clap_remote_controls_page;

fn create_remote_controls() -> Vec<clap_remote_controls_page> {
    vec![
        clap_remote_controls_page {
            section_name: b"Main\0".as_ptr() as *const _,
            page_name: b"Gain\0".as_ptr() as *const _,
            param_ids: &[0, std::ptr::null()][0] as *const _,
            is_for_preset: false,
        },
    ]
}
```

---

### **3.5 Note Ports (`clap.note-ports`)**

#### MIDI and Note Events
Define note ports for MIDI and polyphonic modulation:

```rust
use clap_sys::note_ports::clap_note_port_info;

fn note_ports() -> Vec<clap_note_port_info> {
    vec![
        clap_note_port_info {
            id: 0,
            supported_dialects: clap_sys::note_dialect::CLAP_NOTE_DIALECT_MIDI,
            preferred_dialect: clap_sys::note_dialect::CLAP_NOTE_DIALECT_MIDI,
            name: b"MIDI In\0".as_ptr() as *const _,
        },
    ]
}
```

#### Handling Note Events
Process note events in the audio thread:

```rust
unsafe fn process_note_events(instance: &mut Instance, events: &[clap_event_note]) {
    for event in events {
        match event.header.event_type {
            clap_sys::events::CLAP_EVENT_NOTE_ON => {
                // Handle note-on
            }
            clap_sys::events::CLAP_EVENT_NOTE_OFF => {
                // Handle note-off
            }
            _ => {}
        }
    }
}
```

---

### **3.6 Voice Info (`clap.voice-info`)**

#### Polyphony and Voice Management
Define voice capabilities for polyphonic plugins:

```rust
use clap_sys::voice_info::clap_voice_info;

fn voice_info() -> clap_voice_info {
    clap_voice_info {
        voice_count: 16,
        voice_capacity: 16,
        flags: clap_sys::voice_info::CLAP_VOICE_INFO_SUPPORTS_OVERLAPPING_NOTES,
    }
}
```

---

### **3.7 Latency and Tail (`clap.latency`, `clap.tail`)**

#### Latency Compensation
Report plugin latency to the host:

```rust
fn latency() -> u32 {
    128 // Samples
}
```

#### Infinite Tail for Reverb/Delay
Define an infinite tail for effects like reverb:

```rust
fn tail() -> u32 {
    clap_sys::tail::CLAP_TAIL_INFINITE
}
```

---

### **3.8 Tuning (`clap.tuning`)**

#### Microtonal Scales
Support custom tuning systems:

```rust
use clap_sys::tuning::clap_tuning;

fn tuning() -> clap_tuning {
    clap_tuning {
        tuning: b"12-TET\0".as_ptr() as *const _,
        is_dynamic: false,
    }
}
```

---

## **4. Advanced Topics**

### **4.1 Threading and Real-Time Safety**

#### Audio Thread vs. GUI Thread
- **Never block the audio thread** (e.g., no file I/O, no allocations).
- Use **atomic variables** or **message passing** for communication:

```rust
use std::sync::atomic::{AtomicBool, Ordering};

static GUI_DIRTY: AtomicBool = AtomicBool::new(false);

fn process_audio() {
    if GUI_DIRTY.load(Ordering::Relaxed) {
        // Update GUI
        GUI_DIRTY.store(false, Ordering::Relaxed);
    }
}
```

---

### **4.2 Debugging and Testing**

#### CLAP Validator
Validate your plugin with the official CLAP validator:

```bash
clap-validator path/to/your/plugin.clap
```

#### Logging
Use the `log` crate for debugging:

```rust
use log::info;

fn process_audio() {
    info!("Processing audio block");
}
```

#### Unit Tests
Test your plugin logic:

```rust
#[test]
fn test_gain_parameter() {
    let params = MyParams::default();
    assert_eq!(params.gain.value(), 0.5);
}
```

---

## **5. Example: A Complete CLAP Plugin**

For a complete example, see the `examples/` directory in this repository.

---

## **6. FAQ**

### **How do I add a new CLAP extension?**
1. Implement the corresponding **extension trait** in `aura-clap`.
2. Register the extension in the `plugin_get_extension` function.

### **How do I optimize performance?**
- Use `#[inline]` for critical paths.
- Avoid heap allocations in the audio thread.
- Use **FemtoVG** for hardware-accelerated UI rendering.

### **How do I debug GUI issues?**
- Use **Slint previews** for rapid UI iteration.
- Enable OpenGL debugging with `export MESA_DEBUG=1` (Linux/macOS).

### **How do I handle MIDI CC?**
- Use the `clap.note-ports` extension for MIDI input.
- Map MIDI CC to parameters using `clap.remote-controls`.

---

## **7. Further Reading**
- [CLAP Specification](https://github.com/free-audio/clap)
- [AURA Documentation](https://docs.rs/lx-aura)
- [Slint UI Framework](https://slint.dev/)