//! MIDI input via midir → SPSC ring buffer → audio thread.

use midir::{Ignore, MidiInput, MidiInputConnection};

use crate::events::{Queue, RawMidi};

fn input(name: &str) -> Result<MidiInput, String> {
    let mut mi = MidiInput::new(name).map_err(|e| format!("midi init: {e}"))?;
    mi.ignore(Ignore::SysexAndActiveSense);
    Ok(mi)
}

/// Names of the available MIDI input ports.
#[must_use]
pub fn port_names() -> Vec<String> {
    let Ok(mi) = input("aura-host-list") else {
        return Vec::new();
    };
    mi.ports()
        .iter()
        .map(|p| mi.port_name(p).unwrap_or_else(|e| format!("<{e}>")))
        .collect()
}

pub fn list_ports() {
    let names = port_names();
    println!("{} MIDI input port(s):", names.len());
    for (i, name) in names.iter().enumerate() {
        println!("  [{i}] {name}");
    }
}

/// Open the first port whose name contains `want` (or the first port when
/// `want` is `None`) and feed it into `queue`. The returned connection must be
/// kept alive — dropping it closes the port.
pub fn open(want: Option<&str>, queue: &Queue<RawMidi>) -> Result<MidiInputConnection<()>, String> {
    let mi = input("aura-host")?;
    let ports = mi.ports();
    let port = ports
        .iter()
        .find(|p| match want {
            None => true,
            Some(w) => mi
                .port_name(p)
                .is_ok_and(|n| n.to_lowercase().contains(&w.to_lowercase())),
        })
        .ok_or_else(|| match want {
            Some(w) => format!("no MIDI input port matching {w:?}"),
            None => "no MIDI input ports".to_string(),
        })?
        .clone();

    let name = mi.port_name(&port).unwrap_or_else(|_| "?".into());
    let tx = Queue::clone(queue);

    let conn = mi
        .connect(
            &port,
            "aura-host-in",
            move |_stamp, msg, ()| {
                // Sysex is filtered by Ignore; anything else fits in 3 bytes.
                if msg.is_empty() || msg[0] >= 0xF0 {
                    return;
                }
                let mut raw: RawMidi = [0; 3];
                for (dst, src) in raw.iter_mut().zip(msg) {
                    *dst = *src;
                }
                // Queue full = we are far behind; dropping is better than blocking.
                let _ = tx.push(raw);
            },
            (),
        )
        .map_err(|e| format!("midi connect: {e}"))?;

    println!("MIDI in: {name}");
    Ok(conn)
}
