use aura_baseview::slint_window::SlintWindow;
use baseview::dpi::LogicalSize;
use baseview::{
    Event, EventStatus, HandlerError, Window, WindowHandler, WindowSettings, gl::GlConfig,
};
use baseview::{WindowContext, WindowSize};
use std::cell::{Cell, RefCell};
use std::{num::NonZeroU32, sync::mpsc::channel, thread::spawn};

use crate::gui::AppWindow;

mod gui;

struct ParentWindowHandler {
    surface: RefCell<softbuffer::Surface<WindowContext, WindowContext>>,
    damaged: Cell<bool>,

    _child_window: Option<Window>,
}

impl ParentWindowHandler {
    pub fn new(window: WindowContext) -> Self {
        let ctx = softbuffer::Context::new(window.clone()).unwrap();
        let mut surface = softbuffer::Surface::new(&ctx, window.clone()).unwrap();
        let size = window.size().physical;
        surface
            .resize(
                NonZeroU32::new(size.width).unwrap(),
                NonZeroU32::new(size.height).unwrap(),
            )
            .unwrap();

        let options = WindowSettings::new()
            .with_size(LogicalSize::new(256, 256))
            .with_title("baseview child")
            .with_gl_config(GlConfig {
                alpha_bits: 8,
                ..GlConfig::default()
            });

        // to simulate a message to somewhere else
        let (rx, tx) = channel::<f32>();
        let _ = spawn(move || {
            while let Ok(f) = tx.recv() {
                println!("received: {f}");
            }
        });

        let child_window = SlintWindow::open_parented(
            &window,
            options,
            Some(rx),
            move |rx| {
                let rx = rx.take().unwrap();
                let component = AppWindow::new().unwrap();
                component.on_begin_drag(|| eprintln!("on_gain_begin_drag"));
                component.on_changed(move |f| {
                    let _ = rx.send(f);
                });
                component.on_end_drag(|| eprintln!("on_gain_end_drag"));
                component
            },
            |_, _| {},
        )
        .expect("failed to open child window");
        Self {
            surface: surface.into(),
            damaged: true.into(),
            _child_window: Some(child_window.window),
        }
    }
}

impl WindowHandler for ParentWindowHandler {
    fn on_frame(&self) -> Result<(), HandlerError> {
        let mut ref_cell = self.surface.borrow_mut();
        let mut buf = ref_cell.buffer_mut().unwrap();
        if self.damaged.get() {
            buf.fill(0xFFAAAAAA);
            self.damaged.set(false);
        }
        buf.present().unwrap();
        Ok(())
    }

    fn resized(&self, new_size: WindowSize) -> Result<(), HandlerError> {
        println!("Parent Resized: {new_size:?}");

        if let (Some(width), Some(height)) = (
            NonZeroU32::new(new_size.physical.width),
            NonZeroU32::new(new_size.physical.height),
        ) {
            self.surface.borrow_mut().resize(width, height).unwrap();
            self.damaged.set(true);
        }
        Ok(())
    }

    fn on_event(&self, event: Event) -> EventStatus {
        match event {
            Event::Mouse(e) => println!("Parent Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Parent Keyboard event: {:?}", e),
            Event::Window(e) => println!("Parent Window event: {:?}", e),
            _ => {}
        }

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowSettings::new().with_size(LogicalSize::new(512.0, 512.0));

    let window = Window::create(window_open_options, |w| Ok(ParentWindowHandler::new(w)))
        .expect("failed to create parent window");
    window
        .run_until_closed()
        .expect("failed to run parent window");
}
