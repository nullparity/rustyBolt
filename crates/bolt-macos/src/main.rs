//! A native macOS window for rustyBolt.
//!
//! AppKit gives the window and the controls. WKWebView gives the login page.
//! The shell holds user interface code only. It calls `bolt-core` for the
//! sessions, the characters, the install and the launch.
//!
//! Every network call and every install runs on a worker thread. The worker
//! posts the result to the main thread with
//! `performSelectorOnMainThread:withObject:waitUntilDone:` on a helper object
//! that carries a boxed Rust closure.

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{
    define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSButton, NSPopUpButton, NSTextField, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, NSObject, NSObjectNSThreadPerformAdditions, NSObjectProtocol, NSPoint, NSRect,
    NSSize, NSString, NSURL, NSURLRequest,
};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKWebView,
    WKWebViewConfiguration,
};

use bolt_auth::{Action, AuthConfig, Character, LoginFlow, Session};
use bolt_core::{
    launch, plan, ClientKind, Config, CredentialSource, GameCredentials, GcChoice, HttpAuth,
    Installer, LaunchRequest, Paths, SessionStore, TuningConfig, UsageStore,
};

mod advanced;

/// Makes a rectangle from an origin and a size.
pub(crate) fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}

/// Shows the object as a plain Objective-C object.
pub(crate) fn as_any<T: Message>(object: &T) -> &AnyObject {
    // SAFETY: Every Objective-C object has the same layout.
    unsafe { &*(object as *const T as *const AnyObject) }
}

/// Shows the error of a core call as text.
pub(crate) fn describe(error: &bolt_core::CoreError) -> String {
    format!("{error}")
}

/// Shows the active tuning as one line of text.
fn tuning_text(tuning: &TuningConfig) -> String {
    if !tuning.enabled {
        return "Tuning: off".to_string();
    }
    let mut parts = Vec::new();
    match (&tuning.heap_min, &tuning.heap_max) {
        (Some(min), Some(max)) if min == max => parts.push(format!("{min} heap")),
        (Some(min), Some(max)) => parts.push(format!("{min} to {max} heap")),
        (Some(min), None) => parts.push(format!("{min} min heap")),
        (None, Some(max)) => parts.push(format!("{max} heap")),
        (None, None) => {}
    }
    if let Some(stack) = &tuning.stack_size {
        parts.push(format!("{stack} stack"));
    }
    parts.push(
        match tuning.garbage_collector {
            GcChoice::Default => "default GC",
            GcChoice::Z => "ZGC",
            GcChoice::G1 => "G1",
            GcChoice::Parallel => "parallel GC",
        }
        .to_string(),
    );
    if tuning.aot_cache {
        parts.push("AOT cache".to_string());
    }
    if tuning.compact_object_headers {
        parts.push("compact headers".to_string());
    }
    if tuning.string_deduplication {
        parts.push("string deduplication".to_string());
    }
    if tuning.native_access {
        parts.push("native access".to_string());
    }
    if tuning.gc_log {
        parts.push("GC log".to_string());
    }
    if tuning.java2d_metal {
        parts.push("Metal".to_string());
    }
    format!("Tuning: on, {}", parts.join(", "))
}

/// A handle to an object that lives on the main thread.
///
/// The holder uses the handle on the main thread only. The object must stay
/// alive while a handle exists.
pub(crate) struct MainRef<T>(*const T);

impl<T> Clone for MainRef<T> {
    fn clone(&self) -> MainRef<T> {
        MainRef(self.0)
    }
}

impl<T> Copy for MainRef<T> {}

// SAFETY: The holder never touches the object off the main thread. It only
// carries the address across a thread boundary and posts a main thread call.
unsafe impl<T> Send for MainRef<T> {}
unsafe impl<T> Sync for MainRef<T> {}

impl<T> MainRef<T> {
    /// Keeps the address of an object that stays alive.
    pub(crate) fn new(object: &T) -> MainRef<T> {
        MainRef(object as *const T)
    }

    /// Gives the object back.
    ///
    /// # Safety
    ///
    /// The caller must run on the main thread. The object must be alive.
    pub(crate) unsafe fn get(&self) -> &T {
        // SAFETY: The caller keeps the object alive.
        unsafe { &*self.0 }
    }
}

/// The start of a block. The invoke pointer is the code of the block.
#[repr(C)]
struct BlockLiteral {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
    descriptor: *const c_void,
}

/// Runs the decision handler of the navigation delegate.
///
/// # Safety
///
/// The handler must be the block of the delegate call.
unsafe fn answer(handler: *mut c_void, policy: WKNavigationActionPolicy) {
    // SAFETY: The caller gives the block of the delegate call.
    let block = handler as *const BlockLiteral;
    // SAFETY: The invoke pointer of a block has this shape.
    let invoke: unsafe extern "C" fn(*mut BlockLiteral, isize) =
        unsafe { std::mem::transmute((*block).invoke) };
    // SAFETY: The block is alive for the duration of the call.
    unsafe { invoke(handler as *mut BlockLiteral, policy.0) };
}

/// The text of the URL of a navigation action.
fn action_url(action: &WKNavigationAction) -> String {
    // SAFETY: The action belongs to the delegate call.
    let request = unsafe { action.request() };
    request
        .URL()
        .and_then(|url| url.absoluteString())
        .map(|text| text.to_string())
        .unwrap_or_default()
}

/// The posted call of a main thread hop.
pub(crate) struct MainCallIvars {
    call: Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
}

define_class!(
    // SAFETY: The superclass NSObject has no subclassing requirements.
    // SAFETY: `MainCall` does not implement `Drop`.
    #[unsafe(super = NSObject)]
    #[ivars = MainCallIvars]
    pub(crate) struct MainCall;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for MainCall {}

    impl MainCall {
        /// Runs the posted call. The main thread calls this method.
        #[unsafe(method(runCall))]
        fn run_call(&self) {
            let call = self.ivars().call.lock().unwrap().take();
            if let Some(call) = call {
                call();
            }
        }
    }
);

impl MainCall {
    /// Runs `call` on the main thread.
    pub(crate) fn post(call: impl FnOnce() + Send + 'static) {
        let this = MainCall::alloc().set_ivars(MainCallIvars {
            call: Mutex::new(Some(Box::new(call))),
        });
        let this: Retained<MainCall> = unsafe { msg_send![super(this), init] };
        // SAFETY: The selector `runCall` takes no argument. The run loop
        // holds the object until the selector runs.
        unsafe {
            this.performSelectorOnMainThread_withObject_waitUntilDone(sel!(runCall), None, false);
        }
    }
}

/// Shows text in the status field of the main window.
pub(crate) fn post_status(app: MainRef<AppDelegate>, text: String) {
    MainCall::post(move || {
        // SAFETY: The posted call runs on the main thread.
        let app = unsafe { app.get() };
        app.set_status(&text);
    });
}

/// The state of the login flow. The worker thread holds the same value.
struct LoginState {
    config: AuthConfig,
    flow: LoginFlow,
}

/// A result of the login worker.
enum LoginMsg {
    /// Load this URL in the web view.
    Navigate(String),
    /// The flow is complete.
    Done(Session),
    /// The flow stopped with this text.
    Failed(String),
}

/// The delegate of the login web view.
struct LoginIvars {
    state: Arc<Mutex<LoginState>>,
    app: MainRef<AppDelegate>,
    web: Mutex<Option<Retained<WKWebView>>>,
    window: Mutex<Option<Retained<NSWindow>>>,
}

define_class!(
    // SAFETY: The superclass NSObject has no subclassing requirements.
    // SAFETY: `LoginDelegate` does not implement `Drop`.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = LoginIvars]
    struct LoginDelegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for LoginDelegate {}

    // SAFETY: Every method of the protocol is optional.
    unsafe impl WKNavigationDelegate for LoginDelegate {}

    impl LoginDelegate {
        /// Reads every URL that the web view reaches.
        ///
        /// The action `Ignore` lets the page load. Any other action cancels
        /// the navigation, because the shell drives the flow itself.
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        unsafe fn decide(
            &self,
            _web_view: &WKWebView,
            navigation_action: &WKNavigationAction,
            decision_handler: *mut c_void,
        ) {
            let url = action_url(navigation_action);
            let step = {
                let mut state = self.ivars().state.lock().unwrap();
                state.flow.on_navigation(&url)
            };
            match step {
                Ok(Action::Ignore) => {
                    // SAFETY: The handler belongs to this call.
                    unsafe { answer(decision_handler, WKNavigationActionPolicy::Allow) };
                }
                Ok(next) => {
                    // SAFETY: The handler belongs to this call.
                    unsafe { answer(decision_handler, WKNavigationActionPolicy::Cancel) };
                    self.run_steps(next);
                }
                Err(error) => {
                    // SAFETY: The handler belongs to this call.
                    unsafe { answer(decision_handler, WKNavigationActionPolicy::Cancel) };
                    self.report(&format!("login error: {error}"));
                }
            }
        }
    }
);

impl LoginDelegate {
    /// Opens the login window and starts the authorization URL.
    fn open(
        mtm: MainThreadMarker,
        auth: &AuthConfig,
        app: MainRef<AppDelegate>,
    ) -> Retained<LoginDelegate> {
        let state = Arc::new(Mutex::new(LoginState {
            config: auth.clone(),
            flow: LoginFlow::new(auth.clone()),
        }));

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 560.0, 720.0),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(ns_string!("Log in"));

        let configuration = unsafe { WKWebViewConfiguration::new(mtm) };
        let web = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                rect(0.0, 0.0, 560.0, 720.0),
                &configuration,
            )
        };
        if let Some(content) = window.contentView() {
            content.addSubview(&web);
        }

        let this = Self::alloc(mtm).set_ivars(LoginIvars {
            state,
            app,
            web: Mutex::new(Some(web)),
            window: Mutex::new(Some(window)),
        });
        let this: Retained<LoginDelegate> = unsafe { msg_send![super(this), init] };

        {
            let guard = this.ivars().web.lock().unwrap();
            if let Some(web) = guard.as_ref() {
                unsafe { web.setNavigationDelegate(Some(ProtocolObject::from_ref(&*this))) };
            }
        }

        let url = {
            let state = this.ivars().state.lock().unwrap();
            state.flow.authorize_url()
        };
        this.load(&url);

        {
            let guard = this.ivars().window.lock().unwrap();
            if let Some(window) = guard.as_ref() {
                window.center();
                window.makeKeyAndOrderFront(None);
            }
        }

        this
    }

    /// Loads a URL in the web view.
    fn load(&self, url: &str) {
        let Some(url) = NSURL::URLWithString(&NSString::from_str(url)) else {
            self.report("the login URL is not valid");
            return;
        };
        let request = NSURLRequest::requestWithURL(&url);
        let guard = self.ivars().web.lock().unwrap();
        if let Some(web) = guard.as_ref() {
            unsafe { web.loadRequest(&request) };
        }
    }

    /// Returns one line about the login window and the web view.
    fn describe(&self) -> String {
        let window = self.ivars().window.lock().unwrap();
        let web = self.ivars().web.lock().unwrap();
        let stage = {
            let state = self.ivars().state.lock().unwrap();
            format!("{:?}", state.flow.stage())
        };
        match (window.as_ref(), web.as_ref()) {
            (Some(window), Some(web)) => {
                let url = unsafe { web.URL() }
                    .map(|url| url.absoluteString().map(|s| s.to_string()))
                    .flatten()
                    .unwrap_or_else(|| "no url yet".to_string());
                let frame = window.frame();
                format!(
                    "login window \"{}\" visible={} size={}x{} stage={} url={}",
                    window.title(),
                    window.isVisible(),
                    frame.size.width,
                    frame.size.height,
                    stage,
                    url,
                )
            }
            _ => "the login window has no web view".to_string(),
        }
    }

    /// Runs the network steps of the flow on a worker thread.
    fn run_steps(&self, first: Action) {
        let state = Arc::clone(&self.ivars().state);
        let me = MainRef::new(self);
        thread::spawn(move || {
            let mut action = first;
            loop {
                match action {
                    Action::Ignore => return,
                    Action::Navigate { url } => {
                        post_login(me, LoginMsg::Navigate(url));
                        return;
                    }
                    Action::Done(session) => {
                        post_login(me, LoginMsg::Done(session));
                        return;
                    }
                    other => {
                        let next = {
                            let mut held = state.lock().unwrap();
                            let LoginState { config, flow } = &mut *held;
                            let driver = HttpAuth::new(config);
                            driver.advance(flow, other)
                        };
                        match next {
                            Ok(step) => action = step,
                            Err(error) => {
                                post_login(me, LoginMsg::Failed(describe(&error)));
                                return;
                            }
                        }
                    }
                }
            }
        });
    }

    /// Runs a result of the worker on the main thread.
    fn handle(&self, message: LoginMsg) {
        match message {
            LoginMsg::Navigate(url) => self.load(&url),
            LoginMsg::Failed(text) => self.report(&text),
            LoginMsg::Done(session) => {
                // SAFETY: This method runs on the main thread.
                let app = unsafe { self.ivars().app.get() };
                app.session_saved(session);
                let window = self.ivars().window.lock().unwrap().take();
                if let Some(window) = window {
                    window.close();
                }
                let me = self.ivars().app;
                MainCall::post(move || {
                    // SAFETY: The posted call runs on the main thread.
                    let app = unsafe { me.get() };
                    app.login_finished();
                });
            }
        }
    }

    /// Shows an error in the main window.
    fn report(&self, text: &str) {
        // SAFETY: This method runs on the main thread.
        let app = unsafe { self.ivars().app.get() };
        app.set_status(&format!("Login failed: {text}"));
    }
}

/// Posts a login result to the main thread.
fn post_login(delegate: MainRef<LoginDelegate>, message: LoginMsg) {
    MainCall::post(move || {
        // SAFETY: The posted call runs on the main thread.
        let delegate = unsafe { delegate.get() };
        delegate.handle(message);
    });
}

/// The user interface state of the main window.
struct AppState {
    paths: Paths,
    config: Config,
    auth: AuthConfig,
    store: SessionStore,
    usage: UsageStore,
    characters: Vec<Character>,
    session_popup: Retained<NSPopUpButton>,
    character_popup: Retained<NSPopUpButton>,
    status: Retained<NSTextField>,
    tuning: Retained<NSTextField>,
}

/// Orders the loaded characters so the recent one comes first.
fn ordered_characters(usage: &UsageStore, characters: &[Character], window: u64) -> Vec<Character> {
    usage
        .order(characters, window, |item| item.account_id.as_str())
        .into_iter()
        .cloned()
        .collect()
}

/// Fills the character pop up button and selects the first item.
fn fill_characters(state: &mut AppState) {
    let ordered = ordered_characters(
        &state.usage,
        &state.characters,
        state.config.usage_recent_window_secs,
    );
    state.character_popup.removeAllItems();
    for character in &ordered {
        state
            .character_popup
            .addItemWithTitle(&NSString::from_str(&character.display_name));
    }
    if !ordered.is_empty() {
        state.character_popup.selectItemAtIndex(0);
    }
    state.characters = ordered;
}

/// The instance variables of the application delegate.
pub(crate) struct AppIvars {
    state: Mutex<Option<AppState>>,
    window: Mutex<Option<Retained<NSWindow>>>,
    login: Mutex<Option<Retained<LoginDelegate>>>,
    advanced: Mutex<Option<Retained<advanced::AdvancedDelegate>>>,
}

impl AppIvars {
    /// Makes empty instance variables. `build` fills them.
    fn empty() -> AppIvars {
        AppIvars {
            state: Mutex::new(None),
            window: Mutex::new(None),
            login: Mutex::new(None),
            advanced: Mutex::new(None),
        }
    }
}

define_class!(
    // SAFETY: The superclass NSObject has no subclassing requirements.
    // SAFETY: `AppDelegate` does not implement `Drop`.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppIvars]
    pub(crate) struct AppDelegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for AppDelegate {}

    // SAFETY: `NSApplicationDelegate` has no safety requirements.
    unsafe impl NSApplicationDelegate for AppDelegate {
        /// Builds the main window.
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &objc2_foundation::NSNotification) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            self.build(mtm);
        }

        /// Stops the application when the main window closes.
        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate(&self, _application: &NSApplication) -> bool {
            true
        }
    }

    impl AppDelegate {
        /// Opens the login window.
        #[unsafe(method(loginClicked:))]
        fn login_clicked(&self, _sender: Option<&AnyObject>) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let auth = {
                let guard = self.ivars().state.lock().unwrap();
                match guard.as_ref() {
                    Some(state) => state.auth.clone(),
                    None => return,
                }
            };
            let window = LoginDelegate::open(mtm, &auth, MainRef::new(self));
            *self.ivars().login.lock().unwrap() = Some(window);
            self.set_status("Waiting for the login page.");
        }

        /// Removes the selected session and saves the store.
        #[unsafe(method(logoutClicked:))]
        fn logout_clicked(&self, _sender: Option<&AnyObject>) {
            let mut guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_mut() else {
                return;
            };
            let index = state.session_popup.indexOfSelectedItem();
            if index < 0 || index as usize >= state.store.sessions().len() {
                return;
            }
            let sub = state.store.sessions()[index as usize].sub.clone();
            state.store.remove(&sub);
            let result = state.store.save();
            let count = state.store.sessions().len();
            let text = match result {
                Ok(()) => format!("Logged out. {count} session(s) remain."),
                Err(error) => format!("Could not save the sessions: {error}"),
            };
            state.status.setStringValue(&NSString::from_str(&text));
            drop(guard);
            self.refresh_sessions();
        }

        /// Loads the characters of the selected session.
        #[unsafe(method(sessionChanged:))]
        fn session_changed(&self, _sender: Option<&AnyObject>) {
            self.load_characters();
        }

        /// Installs the client when it is absent, then starts it.
        #[unsafe(method(launchClicked:))]
        fn launch_clicked(&self, _sender: Option<&AnyObject>) {
            let job = {
                let guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_ref() else {
                    return;
                };
                let index = state.session_popup.indexOfSelectedItem();
                if index < 0 || index as usize >= state.store.sessions().len() {
                    let text = NSString::from_str("Select a session first.");
                    state.status.setStringValue(&text);
                    return;
                }
                let session = &state.store.sessions()[index as usize];
                let character = state
                    .character_popup
                    .indexOfSelectedItem()
                    .try_into()
                    .ok()
                    .and_then(|index: usize| state.characters.get(index));
                let java = state
                    .config
                    .java_path
                    .clone()
                    .or_else(|| bolt_jdk::select(11).map(|runtime| runtime.path));
                LaunchJob {
                    paths: state.paths.clone(),
                    java,
                    template: state.config.runelite_launch_command.clone(),
                    credentials_source: state.config.credential_source.clone(),
                    session_id: session.session_id.clone(),
                    character_id: character.map(|item| item.account_id.clone()).unwrap_or_default(),
                    display_name: character
                        .map(|item| item.display_name.clone())
                        .unwrap_or_else(|| session.display_name.clone()),
                }
            };

            let me = MainRef::new(self);
            post_status(me, "Starting RuneLite...".to_string());
            thread::spawn(move || {
                let result = run_launch(&job, me);
                let account_id = job.character_id.clone();
                MainCall::post(move || {
                    // SAFETY: The posted call runs on the main thread.
                    let app = unsafe { me.get() };
                    match result {
                        Ok(pid) => {
                            app.set_status(&format!("RuneLite started. Process {pid}."));
                            if !account_id.is_empty() {
                                app.record_use(&account_id);
                            }
                        }
                        Err(text) => app.set_status(&text),
                    }
                });
            });
        }

        /// Opens the advanced options window.
        #[unsafe(method(advancedClicked:))]
        fn advanced_clicked(&self, _sender: Option<&AnyObject>) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            self.open_advanced(mtm, false);
        }
    }
);

impl AppDelegate {
    /// Makes the delegate with empty instance variables.
    fn new(mtm: MainThreadMarker) -> Retained<AppDelegate> {
        let this = Self::alloc(mtm).set_ivars(AppIvars::empty());
        unsafe { msg_send![super(this), init] }
    }

    /// Builds the main window and the controls.
    fn build(&self, mtm: MainThreadMarker) {
        let Ok(paths) = Paths::resolve() else {
            return;
        };
        let config = Config::load(&paths);
        let auth = AuthConfig::default();
        let store = SessionStore::load(&paths);

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 900.0, 600.0),
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(ns_string!("rustyBolt"));

        let target = Some(as_any(self));

        let heading = NSTextField::labelWithString(ns_string!("rustyBolt launcher"), mtm);
        heading.setFrame(rect(20.0, 545.0, 860.0, 24.0));

        let session_label = NSTextField::labelWithString(ns_string!("Session"), mtm);
        session_label.setFrame(rect(20.0, 512.0, 200.0, 18.0));

        let session_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(20.0, 478.0, 560.0, 30.0),
            false,
        );
        unsafe {
            session_popup.setTarget(target);
            session_popup.setAction(Some(sel!(sessionChanged:)));
        }

        let login_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Log in"),
                target,
                Some(sel!(loginClicked:)),
                mtm,
            )
        };
        login_button.setFrame(rect(600.0, 476.0, 130.0, 32.0));

        let logout_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Log out"),
                target,
                Some(sel!(logoutClicked:)),
                mtm,
            )
        };
        logout_button.setFrame(rect(745.0, 476.0, 135.0, 32.0));

        let character_label = NSTextField::labelWithString(ns_string!("Character"), mtm);
        character_label.setFrame(rect(20.0, 440.0, 200.0, 18.0));

        let character_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(20.0, 406.0, 300.0, 30.0),
            false,
        );

        let launch_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Launch RuneLite"),
                target,
                Some(sel!(launchClicked:)),
                mtm,
            )
        };
        launch_button.setFrame(rect(340.0, 404.0, 220.0, 32.0));

        let advanced_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Advanced…"),
                target,
                Some(sel!(advancedClicked:)),
                mtm,
            )
        };
        advanced_button.setFrame(rect(575.0, 404.0, 150.0, 32.0));

        let status = NSTextField::labelWithString(ns_string!("Ready."), mtm);
        status.setFrame(rect(20.0, 44.0, 860.0, 56.0));

        let tuning = NSTextField::labelWithString(
            &NSString::from_str(&tuning_text(&config.runelite_tuning)),
            mtm,
        );
        tuning.setFrame(rect(20.0, 20.0, 860.0, 20.0));

        if let Some(content) = window.contentView() {
            for view in [
                &*heading as &NSView,
                &*session_label,
                &*session_popup,
                &*login_button,
                &*logout_button,
                &*character_label,
                &*character_popup,
                &*launch_button,
                &*advanced_button,
                &*status,
                &*tuning,
            ] {
                content.addSubview(view);
            }
        }

        let usage = UsageStore::load(&paths);
        *self.ivars().state.lock().unwrap() = Some(AppState {
            paths,
            config,
            auth,
            store,
            usage,
            characters: Vec::new(),
            session_popup,
            character_popup,
            status,
            tuning,
        });

        window.center();
        window.makeKeyAndOrderFront(None);
        *self.ivars().window.lock().unwrap() = Some(window);

        self.refresh_sessions();
        self.set_status("Ready.");

        if std::env::args().any(|argument| {
            argument == "--self-check" || argument == "--self-check-advanced"
        }) {
            self.report_window(mtm);
        }
    }

    /// Prints the state of the window and of every control.
    ///
    /// A person cannot always see the screen of a test machine. This report gives
    /// the same facts as a screenshot: the window, its size and each control.
    fn report_window(&self, mtm: MainThreadMarker) {
        let guard = self.ivars().window.lock().unwrap();
        let window = match guard.as_ref() {
            Some(window) => window,
            None => {
                println!("self-check: no window");
                return;
            }
        };
        let frame = window.frame();
        println!(
            "self-check: window \"{}\" visible={} size={}x{}",
            window.title(),
            window.isVisible(),
            frame.size.width,
            frame.size.height,
        );
        if let Some(content) = window.contentView() {
            for view in content.subviews().iter() {
                // `NSPopUpButton` is a subclass of `NSButton`, so it comes first.
                let title = view
                    .downcast_ref::<NSPopUpButton>()
                    .map(|popup| format!("popup with {} items", popup.numberOfItems()))
                    .or_else(|| {
                        view.downcast_ref::<NSButton>()
                            .map(|button| button.title().to_string())
                    })
                    .or_else(|| {
                        view.downcast_ref::<NSTextField>()
                            .map(|field| field.stringValue().to_string())
                    })
                    .unwrap_or_else(|| "view".to_string());
                let bounds = view.frame();
                println!(
                    "self-check: control \"{}\" at {}x{}",
                    title, bounds.origin.x, bounds.origin.y
                );
            }
        }

        let tuning = {
            let guard = self.ivars().state.lock().unwrap();
            guard
                .as_ref()
                .map(|state| state.tuning.stringValue().to_string())
        };
        match tuning {
            Some(text) => println!("self-check: tuning line \"{text}\""),
            None => println!("self-check: the application has no state"),
        }

        if std::env::args().any(|argument| argument == "--self-check-login") {
            self.report_login(mtm);
        }
        if std::env::args().any(|argument| argument == "--self-check-advanced") {
            self.open_advanced(mtm, true);
        }
    }

    /// Opens the login window and reports its state.
    fn report_login(&self, mtm: MainThreadMarker) {
        let auth = {
            let guard = self.ivars().state.lock().unwrap();
            match guard.as_ref() {
                Some(state) => state.auth.clone(),
                None => {
                    println!("self-check: the application has no state");
                    return;
                }
            }
        };
        let login = LoginDelegate::open(mtm, &auth, MainRef::new(self));
        println!("self-check: {}", login.describe());
        *self.ivars().login.lock().unwrap() = Some(login);
    }

    /// Shows text in the status field.
    pub(crate) fn set_status(&self, text: &str) {
        let guard = self.ivars().state.lock().unwrap();
        if let Some(state) = guard.as_ref() {
            state.status.setStringValue(&NSString::from_str(text));
        }
    }

    /// Opens the advanced options window. `report` prints the self check report.
    pub(crate) fn open_advanced(&self, mtm: MainThreadMarker, report: bool) {
        let paths = {
            let guard = self.ivars().state.lock().unwrap();
            match guard.as_ref() {
                Some(state) => state.paths.clone(),
                None => return,
            }
        };
        let window = advanced::AdvancedDelegate::open(mtm, MainRef::new(self), paths, report);
        *self.ivars().advanced.lock().unwrap() = Some(window);
    }

    /// Stores a new config and shows it in the main window.
    pub(crate) fn apply_config(&self, config: &Config, status: &str) {
        let mut guard = self.ivars().state.lock().unwrap();
        let Some(state) = guard.as_mut() else {
            return;
        };
        state.config = config.clone();
        let tuning = NSString::from_str(&tuning_text(&config.runelite_tuning));
        state.tuning.setStringValue(&tuning);
        state.status.setStringValue(&NSString::from_str(status));
    }

    /// Fills the session list from the store.
    fn refresh_sessions(&self) {
        let mut guard = self.ivars().state.lock().unwrap();
        let Some(state) = guard.as_mut() else {
            return;
        };
        state.session_popup.removeAllItems();
        for session in state.store.sessions() {
            let title = format!("{} ({})", session.display_name, session.suffix);
            state.session_popup.addItemWithTitle(&NSString::from_str(&title));
        }
        state.character_popup.removeAllItems();
        state.characters.clear();
        if !state.store.sessions().is_empty() {
            state.session_popup.selectItemAtIndex(0);
        }
        drop(guard);
        self.load_characters();
    }

    /// Loads the characters of the selected session on a worker thread.
    fn load_characters(&self) {
        let job = {
            let guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_ref() else {
                return;
            };
            let index = state.session_popup.indexOfSelectedItem();
            if index < 0 || index as usize >= state.store.sessions().len() {
                return;
            }
            (
                state.auth.clone(),
                state.store.sessions()[index as usize].session_id.clone(),
            )
        };

        let me = MainRef::new(self);
        post_status(me, "Loading characters...".to_string());
        thread::spawn(move || {
            let result = HttpAuth::new(&job.0).characters(&job.1);
            MainCall::post(move || {
                // SAFETY: The posted call runs on the main thread.
                let app = unsafe { me.get() };
                app.characters_loaded(result);
            });
        });
    }

    /// Fills the character list with the loaded characters.
    fn characters_loaded(&self, result: Result<Vec<Character>, bolt_core::CoreError>) {
        let mut guard = self.ivars().state.lock().unwrap();
        let Some(state) = guard.as_mut() else {
            return;
        };
        match result {
            Ok(characters) => {
                let count = characters.len();
                state.characters = characters;
                fill_characters(state);
                let text = NSString::from_str(&format!("{count} character(s) loaded."));
                state.status.setStringValue(&text);
            }
            Err(error) => {
                state.characters.clear();
                state.character_popup.removeAllItems();
                let text = NSString::from_str(&format!("Could not load the characters: {error}"));
                state.status.setStringValue(&text);
            }
        }
    }

    /// Records one use of a character and fills the character list again.
    fn record_use(&self, account_id: &str) {
        let mut guard = self.ivars().state.lock().unwrap();
        let Some(state) = guard.as_mut() else {
            return;
        };
        state.usage.mark_used(account_id);
        let saved = state.usage.save();
        fill_characters(state);
        if let Err(error) = saved {
            let text = NSString::from_str(&format!("Could not save the use records: {error}"));
            state.status.setStringValue(&text);
        }
    }

    /// Saves a new session and shows it.
    fn session_saved(&self, session: Session) {
        let mut guard = self.ivars().state.lock().unwrap();
        let Some(state) = guard.as_mut() else {
            return;
        };
        state.store.upsert(session);
        match state.store.save() {
            Ok(()) => {
                let text = NSString::from_str("Login complete.");
                state.status.setStringValue(&text);
            }
            Err(error) => {
                let text = NSString::from_str(&format!("Could not save the session: {error}"));
                state.status.setStringValue(&text);
            }
        }
        drop(guard);
        self.refresh_sessions();
    }

    /// Drops the reference to the closed login window.
    fn login_finished(&self) {
        *self.ivars().login.lock().unwrap() = None;
    }
}

/// One launch request. Every value is owned, so a worker thread can hold it.
struct LaunchJob {
    paths: Paths,
    java: Option<PathBuf>,
    template: Option<String>,
    credentials_source: CredentialSource,
    session_id: String,
    character_id: String,
    display_name: String,
}

/// Reads the login values of one launch.
///
/// A saved session gives the values of the selected character. A command
/// source runs the credential command first, and a status line tells the user
/// that a secret manager can ask for approval.
fn credentials_for(job: &LaunchJob, app: MainRef<AppDelegate>) -> Result<GameCredentials, String> {
    match &job.credentials_source {
        CredentialSource::Session => Ok(GameCredentials {
            session_id: job.session_id.clone(),
            character_id: job.character_id.clone(),
            display_name: job.display_name.clone(),
        }),
        CredentialSource::Command(command) => {
            post_status(
                app,
                format!(
                    "Reading the login values with {}. A secret manager can ask for approval.",
                    command.program
                ),
            );
            command
                .fetch(&job.display_name)
                .map_err(|error| describe(&error))
        }
    }
}

/// Installs RuneLite when it is absent, then starts the client.
fn run_launch(job: &LaunchJob, app: MainRef<AppDelegate>) -> Result<u32, String> {
    let installer = Installer::new(&job.paths);
    let kind = ClientKind::RuneLite;
    let client = match installer.installed(kind) {
        Some(client) => client,
        None => {
            let release = installer.latest(kind).map_err(|error| describe(&error))?;
            post_status(
                app,
                format!("Downloading RuneLite {}.", release.version),
            );
            let mut shown = u64::MAX;
            installer
                .install(kind, &release, &mut |done, total| {
                    let step = match total {
                        Some(total) if total > 0 => done.saturating_mul(100) / total,
                        _ => done / (1024 * 1024),
                    };
                    if step != shown {
                        shown = step;
                        let text = match total {
                            Some(total) if total > 0 => {
                                format!("Downloading RuneLite {step} percent.")
                            }
                            _ => format!("Downloading RuneLite {step} megabytes."),
                        };
                        post_status(app, text);
                    }
                })
                .map_err(|error| describe(&error))?
        }
    };

    let java = match job.java.clone() {
        Some(java) => java,
        None => bolt_jdk::select(11)
            .map(|runtime| runtime.path)
            .ok_or_else(|| "No Java runtime of version 11 or newer exists.".to_string())?,
    };

    let credentials = credentials_for(job, app)?;
    let request = LaunchRequest {
        jar: &client.jar,
        kind,
        credentials: Some(&credentials),
        java: Some(&java),
        template: job.template.as_deref(),
        configure: false,
    };
    let planned = plan(&job.paths, &request).map_err(|error| describe(&error))?;
    let program = planned
        .program
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| planned.program.display().to_string());
    post_status(
        app,
        format!("Launching {program} with {} arguments.", planned.args.len()),
    );
    launch(&job.paths, &request).map_err(|error| describe(&error))
}

fn main() {
    let mtm = MainThreadMarker::new().expect("the main thread runs this program");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.activate();
    app.run();
}
