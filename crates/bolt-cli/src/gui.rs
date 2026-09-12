//! Native desktop window driver using wry and tao.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use wry::WebViewBuilder;

use crate::wifi::WifiManager;

#[derive(Debug)]
pub(crate) enum AppEvent {
    Shutdown,
    HideWindow,
    #[allow(dead_code)]
    ShowWindow,
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

fn create_tray_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let width = 32;
    let height = 32;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            let in_bolt = ((4..16).contains(&y) && x >= 14 - (y - 4) / 2 && x <= 22 - (y - 4) / 3)
                || ((14..18).contains(&y) && (8..=24).contains(&x))
                || ((17..28).contains(&y) && x >= 10 + (y - 17) / 2 && x <= 18);
            if in_bolt {
                rgba[idx] = 229;
                rgba[idx + 1] = 169;
                rgba[idx + 2] = 60;
                rgba[idx + 3] = 255;
            }
        }
    }
    Ok(Icon::from_rgba(rgba, width, height)?)
}

pub(crate) fn run_window(
    event_loop: EventLoop<AppEvent>,
    url: &str,
    running: Arc<AtomicBool>,
    wifi_manager: WifiManager,
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

    let tray_menu = Menu::new();
    let open_item = MenuItem::new("Open rustyBolt", true, None);
    let wifi_item = CheckMenuItem::new("Wifi Mode", true, wifi_manager.is_enabled(), None);
    let sep = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit rustyBolt", true, None);
    tray_menu.append_items(&[&open_item, &wifi_item, &sep, &quit_item])?;

    let tray_icon = create_tray_icon().ok();
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("rustyBolt");
    if let Some(icon) = tray_icon {
        tray_builder = tray_builder.with_icon(icon);
    }
    let _tray = tray_builder.build().ok();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100));

        if !running.load(Ordering::SeqCst) {
            *control_flow = ControlFlow::Exit;
            return;
        }

        while let Ok(menu_ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if menu_ev.id == open_item.id() {
                window.set_visible(true);
                window.set_focus();
            } else if menu_ev.id == wifi_item.id() {
                let active = wifi_manager.toggle();
                wifi_item.set_checked(active);
            } else if menu_ev.id == quit_item.id() {
                running.store(false, Ordering::SeqCst);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        while let Ok(tray_ev) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { .. } = tray_ev {
                window.set_visible(true);
                window.set_focus();
            }
        }

        wifi_item.set_checked(wifi_manager.is_enabled());

        match event {
            Event::UserEvent(AppEvent::Shutdown) => {
                running.store(false, Ordering::SeqCst);
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(AppEvent::HideWindow) => {
                window.set_visible(false);
            }
            Event::UserEvent(AppEvent::ShowWindow) => {
                window.set_visible(true);
                window.set_focus();
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                window.set_visible(false);
            }
            _ => {}
        }
    });
}
