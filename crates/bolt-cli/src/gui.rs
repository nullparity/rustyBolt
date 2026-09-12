//! Native desktop window driver using wry and tao.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

#[derive(Debug)]
pub(crate) enum AppEvent {
    Shutdown,
}

pub(crate) fn has_display() -> bool {
    #[cfg(target_os = "macos")]
    return true;

    #[cfg(target_os = "windows")]
    return true;

    #[cfg(target_os = "linux")]
    return std::env::var("DISPLAY").is_ok() || std::env::var("WAYLAND_DISPLAY").is_ok();
}

pub(crate) fn create_event_loop() -> (EventLoop<AppEvent>, EventLoopProxy<AppEvent>) {
    let event_loop = EventLoopBuilder::<AppEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    (event_loop, proxy)
}

pub(crate) fn run_window(
    event_loop: EventLoop<AppEvent>,
    url: &str,
    running: Arc<AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let window = WindowBuilder::new()
        .with_title("rustyBolt")
        .with_inner_size(LogicalSize::new(960.0, 740.0))
        .with_min_inner_size(LogicalSize::new(640.0, 520.0))
        .build(&event_loop)?;

    let builder = WebViewBuilder::new().with_url(url);

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let _webview = builder.build(&window)?;

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window
            .default_vbox()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "no GTK vbox"))?;
        builder.build_gtk(vbox)?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        if !running.load(Ordering::SeqCst) {
            *control_flow = ControlFlow::Exit;
            return;
        }

        match event {
            Event::UserEvent(AppEvent::Shutdown) => {
                running.store(false, Ordering::SeqCst);
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                running.store(false, Ordering::SeqCst);
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
