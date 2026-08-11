use crate::gui::AppWindow;
use aura_baseview::slint_window::SlintWindow;
use baseview::{WindowSettings, dpi::PhysicalSize, gl::GlConfig};
use std::{io::stdin, sync::mpsc::channel, thread::spawn};

mod gui;

fn main() {
    let window_open_options = WindowSettings::new()
        .with_title("slint on Baseview")
        .with_size(PhysicalSize::new(512, 512))
        .with_gl_config(GlConfig {
            alpha_bits: 8,
            ..GlConfig::default()
        });

    let (ss, rs) = channel::<String>();
    let tt = spawn(move || {
        loop {
            let mut buffer = String::new();
            stdin().read_line(&mut buffer).unwrap();
            ss.send(buffer).unwrap();
        }
    });

    let (sf, rf) = channel::<f32>();
    let t = spawn(move || {
        while let Ok(f) = rf.recv() {
            println!("{f}");
        }
    });

    SlintWindow::open_blocking(
        window_open_options,
        rs,
        move |_state| {
            let sf = sf;
            let component = AppWindow::new().unwrap();
            component.on_begin_drag(|| eprintln!("on_gain_begin_drag"));
            component.on_changed(move |f| {
                eprintln!("on_set_value");
                let _ = sf.send(f);
            });
            component.on_end_drag(|| eprintln!("on_gain_end_drag"));
            component
        },
        move |component: &AppWindow, state| {
            if let Ok(s) = state.try_recv() {
                let v = component.get_value();
                let v = (if s == "u\n" {
                    v + 0.1
                } else if s == "d\n" {
                    v - 0.1
                } else {
                    v
                })
                .clamp(0.0, 1.0);
                component.set_value(v);
            }
        },
    )
    .expect("failed to open slint window");
    t.join().unwrap();
    tt.join().unwrap();
}
