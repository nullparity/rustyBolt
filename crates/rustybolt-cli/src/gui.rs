//! Native desktop window driver using wry and tao.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder, TrayIconEvent};
use wry::{NewWindowResponse, WebContext, WebView, WebViewBuilder};

use rustybolt_core::{LoginFlow, Paths};

use crate::platform::{claim_jagex_scheme, release_jagex_scheme};

use crate::configure::{complete_login, LoginOutcome};
use crate::i18n::Lang;
use crate::wifi::WifiManager;

#[derive(Debug)]
pub(crate) enum AppEvent {
    Shutdown,
    HideWindow,
    #[allow(dead_code)]
    ShowWindow,
    /// Authorize URL for a login in the system browser.
    OpenLoginBrowser(String),
    /// Authorize URL for a login in the in-app window.
    OpenLoginWindow(String),
    /// Show the consent page in the login window.
    ShowConsent(String),
    /// The login window hit one of the OAuth redirect targets.
    LoginRedirect(String),
    /// A worker thread finished advancing the login flow.
    LoginOutcome(LoginOutcome),
}

/// The dedicated Jagex login window and its webview.
struct LoginWindow {
    window: Window,
    webview: WebView,
    /// Must outlive the webview on Linux.
    _context: WebContext,
}

/// The profile directory of a webview.
///
/// WebView2 puts its profile next to the executable unless told otherwise,
/// and that fails with "Access is denied" under `Program Files`. The login
/// window gets its own profile, so Jagex cookies stay apart from the
/// dashboard.
fn web_context(paths: &Paths, name: &str) -> WebContext {
    WebContext::new(Some(paths.data_dir.join(name)))
}

/// Builds a webview inside `window` with the given attributes.
fn attach_webview(
    builder: WebViewBuilder<'_>,
    window: &Window,
) -> Result<WebView, Box<dyn std::error::Error>> {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    return Ok(builder.build(window)?);

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window
            .default_vbox()
            .ok_or_else(|| std::io::Error::other("no GTK vbox"))?;
        Ok(builder.build_gtk(vbox)?)
    }
}

#[cfg(target_os = "macos")]
const LOGIN_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";
#[cfg(target_os = "windows")]
const LOGIN_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 Edg/124.0.0.0";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const LOGIN_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.4 Safari/605.1.15";

/// Opens the login window on the authorize URL.
///
/// Navigation is limited to HTTPS pages on Jagex domains. The two OAuth
/// redirect targets are intercepted and sent back as `LoginRedirect`.
fn open_login_window(
    event_loop: &tao::event_loop::EventLoopWindowTarget<AppEvent>,
    proxy: EventLoopProxy<AppEvent>,
    url: &str,
    lang: Lang,
    paths: &Paths,
) -> Result<LoginWindow, Box<dyn std::error::Error>> {
    let window = WindowBuilder::new()
        .with_title(lang.tr("native.login_title"))
        .with_inner_size(LogicalSize::new(520.0, 760.0))
        .with_min_inner_size(LogicalSize::new(420.0, 560.0))
        .build(event_loop)?;

    let mut context = web_context(paths, "webview-login");
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(url)
        // Cloudflare Turnstile stalls on the bare embedded-webview UA.
        .with_user_agent(LOGIN_USER_AGENT)
        .with_navigation_handler(move |nav_url| {
            if rustybolt_security::is_login_redirect(&nav_url) {
                let _ = proxy.send_event(AppEvent::LoginRedirect(nav_url));
                return false;
            }
            let allowed = rustybolt_security::is_allowed_login_navigation(&nav_url);
            if !allowed {
                eprintln!("rustybolt: login window blocked navigation to {nav_url}");
            }
            allowed
        })
        .with_new_window_req_handler(|_url, _features| NewWindowResponse::Deny);

    let webview = attach_webview(builder, &window)?;
    Ok(LoginWindow {
        window,
        webview,
        _context: context,
    })
}

/// Escapes a JSON document so it can sit inside a JS double-quoted string.
fn js_string(json: &str) -> String {
    serde_json::to_string(json).unwrap_or_else(|_| "\"\"".to_string())
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

/// The tray icon, rendered to 64 pixels by `icon/build.sh`. macOS gets the
/// monochrome template from `icon/rustybolt-template.svg`, which the menu
/// bar tints; the other systems get the colour `icon/rustybolt.svg`.
#[cfg(target_os = "macos")]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../../icon/rustybolt-tray-template.png");
#[cfg(not(target_os = "macos"))]
const TRAY_ICON_PNG: &[u8] = include_bytes!("../../../icon/rustybolt-tray.png");

fn create_tray_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let decoder = png::Decoder::new(std::io::Cursor::new(TRAY_ICON_PNG));
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![
        0u8;
        reader
            .output_buffer_size()
            .ok_or("tray icon is too large")?
    ];
    let info = reader.next_frame(&mut buffer)?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buffer[..info.buffer_size()]
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        other => return Err(format!("tray icon has color type {other:?}, not RGB(A)").into()),
    };
    Ok(Icon::from_rgba(rgba, info.width, info.height)?)
}

/// Installs the macOS menu bar and returns it with its Quit item.
///
/// WKWebView takes Cmd+C, Cmd+V, Cmd+A, and Cmd+Z from the Edit menu. Without
/// that menu, those keys do nothing in the text fields of the dashboard.
/// Quit is a normal item, not the predefined one, so it goes through the
/// event loop and releases the `jagex:` scheme like the Quit item of the tray.
#[cfg(target_os = "macos")]
fn install_menu_bar(lang: Lang) -> Result<(Menu, MenuItem), Box<dyn std::error::Error>> {
    use tray_icon::menu::accelerator::{Accelerator, Code, Modifiers};
    use tray_icon::menu::{AboutMetadata, Submenu};

    let quit = MenuItem::new(
        lang.tr("native.tray_quit"),
        true,
        Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyQ)),
    );
    let about = AboutMetadata {
        name: Some("rustyBolt".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        ..AboutMetadata::default()
    };
    let app = Submenu::with_items(
        "rustyBolt",
        true,
        &[
            &PredefinedMenuItem::about(None, Some(about)),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            &quit,
        ],
    )?;
    let edit = Submenu::with_items(
        lang.tr("native.menu_edit"),
        true,
        &[
            &PredefinedMenuItem::undo(None),
            &PredefinedMenuItem::redo(None),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::cut(None),
            &PredefinedMenuItem::copy(None),
            &PredefinedMenuItem::paste(None),
            &PredefinedMenuItem::select_all(None),
        ],
    )?;
    let window = Submenu::with_items(
        lang.tr("native.menu_window"),
        true,
        &[
            &PredefinedMenuItem::minimize(None),
            &PredefinedMenuItem::close_window(None),
        ],
    )?;
    let bar = Menu::with_items(&[&app, &edit, &window])?;
    bar.init_for_nsapp();
    window.set_as_windows_menu_for_nsapp();
    Ok((bar, quit))
}

pub(crate) fn run_window(
    event_loop: EventLoop<AppEvent>,
    url: &str,
    running: Arc<AtomicBool>,
    wifi_manager: WifiManager,
    paths: Arc<Paths>,
    flow_state: Arc<Mutex<Option<LoginFlow>>>,
    lang: Lang,
) -> Result<(), Box<dyn std::error::Error>> {
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("rustyBolt")
        .with_inner_size(LogicalSize::new(960.0, 740.0))
        .with_min_inner_size(LogicalSize::new(640.0, 520.0))
        .build(&event_loop)?;

    let mut context = web_context(&paths, "webview");
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(url)
        .with_navigation_handler(|nav_url| {
            if rustybolt_security::is_allowed_navigation(&nav_url) {
                true
            } else {
                if rustybolt_security::is_allowed_external_url(&nav_url) {
                    crate::configure::open_browser(&nav_url);
                }
                false
            }
        })
        .with_new_window_req_handler(|nav_url, _features| {
            // `target="_blank"` links and window.open never get a webview;
            // allowed Jagex URLs go to the system browser instead.
            if rustybolt_security::is_allowed_external_url(&nav_url) {
                crate::configure::open_browser(&nav_url);
            }
            NewWindowResponse::Deny
        });

    let webview = attach_webview(builder, &window)?;

    let tray_menu = Menu::new();
    let open_item = MenuItem::new(lang.tr("native.tray_open"), true, None);
    let wifi_item = CheckMenuItem::new(
        lang.tr("native.tray_wifi"),
        true,
        wifi_manager.is_enabled(),
        None,
    );
    let sep = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new(lang.tr("native.tray_quit"), true, None);
    tray_menu.append_items(&[&open_item, &wifi_item, &sep, &quit_item])?;

    #[cfg(target_os = "macos")]
    let (_menu_bar, app_quit_item) = install_menu_bar(lang)?;
    #[cfg(target_os = "macos")]
    let app_quit_id = Some(app_quit_item.id().clone());
    #[cfg(not(target_os = "macos"))]
    let app_quit_id: Option<tray_icon::menu::MenuId> = None;

    let tray_icon = create_tray_icon().ok();
    let mut tray_builder = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("rustyBolt");
    if let Some(icon) = tray_icon {
        tray_builder = tray_builder.with_icon(icon);
    }
    #[cfg(target_os = "macos")]
    {
        tray_builder = tray_builder.with_icon_as_template(true);
    }
    // libappindicator-sys panics, rather than errors, when no indicator
    // library is installed. The launcher runs without a tray in that case.
    // The default hook would print a backtrace for that expected panic.
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let built =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tray_builder.build().ok()));
    std::panic::set_hook(hook);
    let _tray = built.unwrap_or_else(|_| {
        eprintln!("rustybolt: no system tray (libayatana-appindicator3 is missing)");
        None
    });

    let mut login: Option<LoginWindow> = None;
    let mut consent: Option<crate::consent::ConsentListener> = None;
    // Whether the current login started in the system browser.
    let mut browser_flow = false;
    // Who held the `jagex:` scheme before this session claimed it.
    // `release_jagex_scheme` gives it back once the login ends.
    let mut jagex_scheme_owner: Option<String> = None;

    event_loop.run(move |event, target, control_flow| {
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
                crate::configure::persist_wifi_mode(&paths, active);
            } else if menu_ev.id == quit_item.id() || app_quit_id.as_ref() == Some(&menu_ev.id) {
                release_jagex_scheme(jagex_scheme_owner.take().as_deref());
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
                release_jagex_scheme(jagex_scheme_owner.take().as_deref());
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
            Event::UserEvent(AppEvent::OpenLoginBrowser(url)) => {
                login = None;
                consent.take();
                browser_flow = true;
                jagex_scheme_owner = claim_jagex_scheme();
                crate::configure::open_browser(&url);
            }
            Event::UserEvent(AppEvent::ShowConsent(url)) if login.is_some() => {
                if let Some(win) = &login {
                    let _ = win.webview.load_url(&url);
                    win.window.set_focus();
                }
            }
            Event::UserEvent(AppEvent::OpenLoginWindow(url) | AppEvent::ShowConsent(url)) => {
                login = None;
                consent.take();
                browser_flow = false;
                match open_login_window(target, proxy.clone(), &url, lang, &paths) {
                    Ok(win) => {
                        win.window.set_focus();
                        login = Some(win);
                    }
                    Err(error) => {
                        eprintln!(
                            "rustybolt: login window failed: {error}. Falling back to browser."
                        );
                        crate::configure::open_browser(&url);
                    }
                }
            }
            Event::UserEvent(AppEvent::LoginRedirect(url)) => {
                // Token and session exchanges block on HTTP; keep them off the UI thread.
                let paths = Arc::clone(&paths);
                let flow_state = Arc::clone(&flow_state);
                let proxy = proxy.clone();
                std::thread::spawn(move || {
                    let outcome = complete_login(&paths, &flow_state, &url);
                    let _ = proxy.send_event(AppEvent::LoginOutcome(outcome));
                });
            }
            Event::UserEvent(AppEvent::LoginOutcome(outcome)) => match outcome {
                LoginOutcome::Navigate(url) => {
                    // Consent needs the session cookie of wherever the login
                    // ran, so it stays in the browser or in the window.
                    if !browser_flow {
                        let _ = proxy.send_event(AppEvent::ShowConsent(url));
                    } else if let Some(listener) = crate::consent::start(proxy.clone()) {
                        consent = Some(listener);
                        crate::configure::open_browser(&url);
                    } else {
                        // Port 80 was free at the readiness check but is gone now.
                        if let Ok(mut lock) = flow_state.lock() {
                            *lock = None;
                        }
                        let _ = webview.evaluate_script(
                            "onNativeLoginError('Port 80 became unavailable. Try again, or log in inside rustyBolt.')",
                        );
                    }
                }
                LoginOutcome::Done(json) => {
                    login = None;
                    consent.take();
                    release_jagex_scheme(jagex_scheme_owner.take().as_deref());
                    window.set_visible(true);
                    window.set_focus();
                    let _ = webview.evaluate_script(&format!("onNativeLogin({json})"));
                }
                LoginOutcome::Error(message) => {
                    login = None;
                    consent.take();
                    release_jagex_scheme(jagex_scheme_owner.take().as_deref());
                    window.set_visible(true);
                    window.set_focus();
                    let _ = webview
                        .evaluate_script(&format!("onNativeLoginError({})", js_string(&message)));
                }
            },
            // macOS hands `jagex:` URLs to the running app through Launch Services.
            Event::Opened { urls } => {
                for opened in urls {
                    let opened = opened.to_string();
                    if rustybolt_security::is_login_redirect(&opened) {
                        let _ = proxy.send_event(AppEvent::LoginRedirect(opened));
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } => {
                if login.as_ref().is_some_and(|w| w.window.id() == window_id) {
                    // The user gave up; drop the pending flow with the window.
                    login = None;
                    release_jagex_scheme(jagex_scheme_owner.take().as_deref());
                    if let Ok(mut lock) = flow_state.lock() {
                        *lock = None;
                    }
                    let _ = webview.evaluate_script("onNativeLoginError(null)");
                } else {
                    window.set_visible(false);
                }
            }
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_embedded_tray_icon_decodes() {
        super::create_tray_icon().expect("tray icon");
    }
}
