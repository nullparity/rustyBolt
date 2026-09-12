//! The advanced options window.
//!
//! The window edits the launcher config: the Java runtime and the launch
//! options. It shows the command line that the current settings produce.
//!
//! The Java discovery runs on a worker thread. Every probe starts
//! `java -version`, so the window opens before the runtimes are known.
//! The worker posts the runtime list to the main thread.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::thread;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Sel};
use objc2::{
    define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{
    NSAlert, NSAlertFirstButtonReturn, NSAlertSecondButtonReturn, NSBackingStoreType, NSButton,
    NSControl, NSOpenPanel, NSPopUpButton, NSScrollView, NSTextField, NSView, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, NSInteger, NSObject, NSObjectProtocol, NSRect, NSSize, NSString,
};

use bolt_core::{
    import_apply, import_plan, plan, ClientKind, Config, CoreError, GcChoice, ImportPlan, Installer,
    LaunchRequest, Paths, RuneLiteHome, TuningConfig,
};
use bolt_jdk::{JavaRuntime, Source};

use crate::{as_any, describe, rect, AppDelegate, MainCall, MainRef};

/// The first Java feature that holds the compact object headers, the string
/// deduplication, the native access and the AOT cache.
const FEATURE_24: u32 = 24;

/// The Java feature that the launcher uses when the probe gives no version.
/// The value agrees with the core crate.
const FALLBACK_FEATURE: u32 = 11;

/// The state value of a check box that is on.
const STATE_ON: NSInteger = 1;

/// The answer of a modal panel that the user accepted.
const MODAL_OK: NSInteger = 1;

/// Shows the source of a runtime as text.
fn source_name(source: &Source) -> &'static str {
    match source {
        Source::Explicit => "Explicit",
        Source::JavaHome => "JAVA_HOME",
        Source::Path => "Path",
        Source::SystemLocation => "System location",
    }
}

/// Shows a runtime as one line of text.
fn runtime_title(runtime: &JavaRuntime) -> String {
    let feature = match &runtime.version {
        Some(version) => format!("feature {}", version.feature),
        None => "feature unknown".to_string(),
    };
    format!(
        "{feature} — {} ({})",
        runtime.path.display(),
        source_name(&runtime.source)
    )
}

/// Reads the state of a check box.
fn is_on(button: &NSButton) -> bool {
    // SAFETY: `NSButton` responds to `state` and returns `NSInteger`.
    let state: NSInteger = unsafe { msg_send![button, state] };
    state == STATE_ON
}

/// Turns a check box on or off.
fn set_on(button: &NSButton, on: bool) {
    let state = if on { STATE_ON } else { 0 };
    // SAFETY: `setState:` takes an integer of every button.
    let _: () = unsafe { msg_send![button, setState: state] };
}

/// Reads a text field. An empty field gives `None`.
fn optional_text(field: &NSTextField) -> Option<String> {
    let text = field.stringValue().to_string();
    let text = text.trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Splits a text field into arguments. ASCII whitespace separates the values.
fn split_args(field: &NSTextField) -> Vec<String> {
    field
        .stringValue()
        .to_string()
        .split_ascii_whitespace()
        .map(|item| item.to_string())
        .collect()
}

/// Shows an optional text value in a field.
fn shown_text(value: &Option<String>) -> Retained<NSString> {
    NSString::from_str(value.as_deref().unwrap_or(""))
}

/// Maps an index of the collector pop up button to a collector.
fn gc_choice(index: NSInteger) -> GcChoice {
    match index {
        1 => GcChoice::Z,
        2 => GcChoice::G1,
        3 => GcChoice::Parallel,
        _ => GcChoice::Default,
    }
}

/// Maps a collector to its index in the pop up button.
fn gc_index(choice: &GcChoice) -> NSInteger {
    match choice {
        GcChoice::Default => 0,
        GcChoice::Z => 1,
        GcChoice::G1 => 2,
        GcChoice::Parallel => 3,
    }
}

/// The tuning value that one check box of the launch options controls.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TuningField {
    Enabled,
    CompactObjectHeaders,
    StringDeduplication,
    NativeAccess,
    AotCache,
    GcLog,
    Java2dMetal,
    LauncherNoJvm,
}

impl TuningField {
    /// The title of the check box. `needs_24` adds the feature rule of the four
    /// options that need feature 24.
    fn title(self, needs_24: bool) -> &'static str {
        match self {
            TuningField::Enabled => "Use the tuned profile",
            TuningField::CompactObjectHeaders => {
                if needs_24 {
                    "Compact object headers (needs feature 24)"
                } else {
                    "Compact object headers"
                }
            }
            TuningField::StringDeduplication => {
                if needs_24 {
                    "String deduplication (needs feature 24)"
                } else {
                    "String deduplication"
                }
            }
            TuningField::NativeAccess => {
                if needs_24 {
                    "Native access (needs feature 24)"
                } else {
                    "Native access"
                }
            }
            TuningField::AotCache => {
                if needs_24 {
                    "AOT cache (needs feature 24)"
                } else {
                    "AOT cache"
                }
            }
            TuningField::GcLog => "GC log",
            TuningField::Java2dMetal => "Metal rendering",
            TuningField::LauncherNoJvm => "Launcher no JVM flag",
        }
    }

    /// Reports if the option needs Java feature 24 or newer.
    fn needs_feature_24(self) -> bool {
        matches!(
            self,
            TuningField::CompactObjectHeaders
                | TuningField::StringDeduplication
                | TuningField::NativeAccess
                | TuningField::AotCache
        )
    }

    /// Reads the value of the option from a tuned profile.
    fn read(self, tuning: &TuningConfig) -> bool {
        match self {
            TuningField::Enabled => tuning.enabled,
            TuningField::CompactObjectHeaders => tuning.compact_object_headers,
            TuningField::StringDeduplication => tuning.string_deduplication,
            TuningField::NativeAccess => tuning.native_access,
            TuningField::AotCache => tuning.aot_cache,
            TuningField::GcLog => tuning.gc_log,
            TuningField::Java2dMetal => tuning.java2d_metal,
            TuningField::LauncherNoJvm => tuning.launcher_nojvm,
        }
    }

    /// Writes the value of the option into a tuned profile.
    fn write(self, tuning: &mut TuningConfig, on: bool) {
        match self {
            TuningField::Enabled => tuning.enabled = on,
            TuningField::CompactObjectHeaders => tuning.compact_object_headers = on,
            TuningField::StringDeduplication => tuning.string_deduplication = on,
            TuningField::NativeAccess => tuning.native_access = on,
            TuningField::AotCache => tuning.aot_cache = on,
            TuningField::GcLog => tuning.gc_log = on,
            TuningField::Java2dMetal => tuning.java2d_metal = on,
            TuningField::LauncherNoJvm => tuning.launcher_nojvm = on,
        }
    }
}

/// One check box of the launch options.
struct Check {
    button: Retained<NSButton>,
    field: TuningField,
}

/// The controls and the values of the advanced window.
struct AdvancedState {
    paths: Paths,
    /// The config of the last read or save. A control value wins over this value.
    config: Config,
    /// The chosen Java runtime. `None` means the automatic choice.
    java_choice: Option<PathBuf>,
    /// The Java runtime list. A worker thread fills it.
    runtimes: Vec<JavaRuntime>,
    java_popup: Retained<NSPopUpButton>,
    feature_field: Retained<NSTextField>,
    checks: Vec<Check>,
    heap_min: Retained<NSTextField>,
    heap_max: Retained<NSTextField>,
    stack_size: Retained<NSTextField>,
    gc_popup: Retained<NSPopUpButton>,
    process_name: Retained<NSTextField>,
    application_name: Retained<NSTextField>,
    extra_jvm_args: Retained<NSTextField>,
    extra_app_args: Retained<NSTextField>,
    preview: Retained<NSView>,
    status: Retained<NSTextField>,
    /// The RuneLite home choice. It feeds `config.runelite_home_kind`.
    home_kind: RuneLiteHome,
    home_popup: Retained<NSPopUpButton>,
    /// Shows the directory that `-Duser.home` names.
    home_label: Retained<NSTextField>,
    import_button: Retained<NSButton>,
    /// The import copies the login files when this check box is on.
    secrets_check: Retained<NSButton>,
    /// A copy runs on a worker thread.
    importing: bool,
}

/// Places the rows of the window from the top down.
struct Rows {
    /// The bottom edge of the row that comes next.
    bottom: f64,
}

impl Rows {
    /// Leaves an 8-point gap after each row.
    fn next(&mut self, height: f64) -> f64 {
        self.bottom -= height;
        let y = self.bottom;
        self.bottom -= 8.0;
        y
    }
}

/// Makes a label.
fn make_label(mtm: MainThreadMarker, text: &str, frame: NSRect) -> Retained<NSTextField> {
    let field = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    field.setFrame(frame);
    field
}

/// Makes an editable text field with one action on the delegate.
fn make_field(
    mtm: MainThreadMarker,
    target: Option<&AnyObject>,
    action: Sel,
    frame: NSRect,
) -> Retained<NSTextField> {
    let field = NSTextField::textFieldWithString(ns_string!(""), mtm);
    field.setFrame(frame);
    unsafe {
        field.setTarget(target);
        field.setAction(Some(action));
    }
    field
}

/// Makes a check box with one action on the delegate.
fn make_check(
    mtm: MainThreadMarker,
    target: Option<&AnyObject>,
    action: Sel,
    title: &str,
    frame: NSRect,
) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::checkboxWithTitle_target_action(
            &NSString::from_str(title),
            target,
            Some(action),
            mtm,
        )
    };
    button.setFrame(frame);
    button
}

/// Makes the read only text view of the preview.
///
/// AppKit holds `NSTextView` in a framework, and this crate does not enable the
/// binding. The class comes from the runtime by name.
fn make_text_view(frame: NSRect) -> Retained<NSView> {
    let class = AnyClass::get(c"NSTextView").expect("AppKit holds the NSTextView class");
    // SAFETY: `NSTextView` is a subclass of `NSView`. The window holds plain text.
    let view: *mut NSView = unsafe { msg_send![class, alloc] };
    let view: *mut NSView = unsafe { msg_send![view, initWithFrame: frame] };
    let _: () = unsafe { msg_send![view, setEditable: false] };
    let _: () = unsafe { msg_send![view, setRichText: false] };
    let _: () = unsafe { msg_send![view, setVerticallyResizable: true] };
    let _: () = unsafe { msg_send![view, setHorizontallyResizable: false] };
    let _: () = unsafe { msg_send![view, setMinSize: NSSize::new(0.0, 0.0)] };
    let _: () = unsafe { msg_send![view, setMaxSize: NSSize::new(frame.size.width, 10000.0)] };
    // SAFETY: The `init` call gives one reference of the new text view.
    unsafe { Retained::from_raw(view) }.expect("the text view is made")
}

fn text_of(view: &NSView) -> String {
    // SAFETY: The object is an `NSTextView`, which has `string`.
    let text: Option<Retained<NSString>> = unsafe { msg_send![view, string] };
    text.map(|text| text.to_string()).unwrap_or_default()
}

fn set_text(view: &NSView, text: &str) {
    let value = NSString::from_str(text);
    // SAFETY: The object is an `NSTextView`, which has `setString:`.
    let _: () = unsafe { msg_send![view, setString: &*value] };
}

/// Puts the chosen runtime of a config into the list when the discovery misses it.
///
/// The window must show a chosen runtime, even when the runtime is absent from
/// the standard locations.
fn keep_chosen(runtimes: &mut Vec<JavaRuntime>, config: &Config) {
    let Some(path) = &config.java_path else {
        return;
    };
    if runtimes.iter().any(|runtime| &runtime.path == path) {
        return;
    }
    runtimes.push(JavaRuntime {
        path: path.clone(),
        home: None,
        version: None,
        source: Source::Explicit,
    });
}

/// Fills the Java pop up button from the runtime list.
fn fill_java_popup(state: &mut AdvancedState) {
    state.java_popup.removeAllItems();
    state
        .java_popup
        .addItemWithTitle(ns_string!("Automatic (best of the system)"));
    for runtime in &state.runtimes {
        state
            .java_popup
            .addItemWithTitle(&NSString::from_str(&runtime_title(runtime)));
    }
    state.java_popup.addItemWithTitle(ns_string!("Choose…"));
    select_java_choice(state);
}

/// Selects the chosen runtime in the Java pop up button.
fn select_java_choice(state: &AdvancedState) {
    let index = state
        .java_choice
        .as_ref()
        .and_then(|path| {
            state
                .runtimes
                .iter()
                .position(|runtime| &runtime.path == path)
        })
        .map(|position| position as NSInteger + 1)
        .unwrap_or(0);
    state.java_popup.selectItemAtIndex(index);
}

/// Reads the Java choice of the pop up button.
fn selected_java(state: &AdvancedState) -> Option<PathBuf> {
    let index = state.java_popup.indexOfSelectedItem();
    if index <= 0 {
        return None;
    }
    state
        .runtimes
        .get((index - 1) as usize)
        .map(|runtime| runtime.path.clone())
}

fn is_system_home(kind: &RuneLiteHome) -> bool {
    matches!(kind, RuneLiteHome::System)
}

/// The order matches the items of the home pop up button.
fn home_index(kind: &RuneLiteHome) -> NSInteger {
    match kind {
        RuneLiteHome::Isolated => 0,
        RuneLiteHome::System => 1,
        RuneLiteHome::Custom(_) => 2,
    }
}

/// Shows a home kind as one word.
fn home_kind_name(kind: &RuneLiteHome) -> &'static str {
    match kind {
        RuneLiteHome::Isolated => "private",
        RuneLiteHome::System => "system",
        RuneLiteHome::Custom(_) => "custom",
    }
}

/// Fills the RuneLite home pop up button from the home kind.
///
/// The custom item appears only when the config names a custom directory.
fn fill_home_popup(state: &mut AdvancedState) {
    state.home_popup.removeAllItems();
    state
        .home_popup
        .addItemWithTitle(ns_string!("Private home (the launcher keeps its own)"));
    state
        .home_popup
        .addItemWithTitle(ns_string!("System home (~/.runelite)"));
    if let RuneLiteHome::Custom(dir) = &state.home_kind {
        let title = format!("Custom home: {}", dir.display());
        state.home_popup.addItemWithTitle(&NSString::from_str(&title));
    }
    state.home_popup.addItemWithTitle(ns_string!("Choose…"));
    state
        .home_popup
        .selectItemAtIndex(home_index(&state.home_kind));
}

fn entry_count(count: usize) -> String {
    if count == 1 {
        "1 entry".to_string()
    } else {
        format!("{count} entries")
    }
}

/// Shows a byte count as short text.
fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Shows a message in a modal alert. It returns the answer of the user.
fn show_alert(mtm: MainThreadMarker, title: &str, body: &str, buttons: &[&str]) -> NSInteger {
    let alert = NSAlert::new(mtm);
    alert.setMessageText(&NSString::from_str(title));
    alert.setInformativeText(&NSString::from_str(body));
    for button in buttons {
        let _ = alert.addButtonWithTitle(&NSString::from_str(button));
    }
    alert.runModal()
}

/// A named path wins. The automatic choice takes the newest runtime of the
/// system.
fn runtime_for<'a>(runtimes: &'a [JavaRuntime], choice: Option<&Path>) -> Option<&'a JavaRuntime> {
    match choice {
        Some(path) => runtimes
            .iter()
            .find(|runtime| runtime.path.as_path() == path),
        None => runtimes
            .iter()
            .filter(|runtime| runtime.version.is_some())
            .max_by_key(|runtime| runtime.version.as_ref().map(|version| version.feature)),
    }
}

/// Shows the feature number of the chosen runtime.
fn feature_text(feature: Option<u32>) -> String {
    match feature {
        Some(feature) => format!("The chosen runtime is feature {feature}."),
        None => "The chosen runtime is not known.".to_string(),
    }
}

/// Puts an argument in quotes when it holds a space.
fn quote(text: &str) -> String {
    if text.contains(' ') {
        format!("\"{text}\"")
    } else {
        text.to_string()
    }
}

/// Shows a program and its arguments as one line.
fn command_line(program: &Path, args: &[String]) -> String {
    let mut parts = vec![quote(&program.display().to_string())];
    parts.extend(args.iter().map(|arg| quote(arg)));
    parts.join(" ")
}

/// Shows the names of the environment values, without any value.
fn env_names(env: &[(String, String)]) -> String {
    if env.is_empty() {
        return "none".to_string();
    }
    env.iter()
        .map(|(name, _)| name.clone())
        .collect::<Vec<String>>()
        .join(", ")
}

/// Builds the preview of a system without an installed RuneLite jar.
fn plain_preview(state: &AdvancedState, config: &Config, feature: u32) -> String {
    let mut lines = vec![
        "No RuneLite jar is installed, so the jar arguments are absent.".to_string(),
    ];
    let tuning_config = &config.runelite_tuning;
    if !tuning_config.enabled {
        lines.push("The tuned profile is off.".to_string());
        lines.push("Command line: java".to_string());
        return lines.join("\n");
    }
    let log_dir = state.paths.cache_dir.join("logs");
    let mut parts = vec!["java".to_string()];
    parts.extend(tuning_config.to_tuning(&log_dir, None).flags(feature));
    parts.extend(tuning_config.dock_args());
    for (key, value) in tuning_config.system_properties() {
        parts.push(format!("-D{key}={value}"));
    }
    lines.push("Command line:".to_string());
    lines.push(parts.join(" "));
    let app_args = tuning_config
        .extra_app_args
        .iter()
        .map(|arg| quote(arg))
        .collect::<Vec<String>>()
        .join(" ");
    lines.push(format!("Application arguments: {app_args}"));
    lines.join("\n")
}

/// Shows the command line that the current settings produce.
fn preview_text(state: &AdvancedState, config: &Config, feature: u32) -> String {
    let installer = Installer::new(&state.paths);
    let Some(client) = installer.installed(ClientKind::RuneLite) else {
        return plain_preview(state, config, feature);
    };
    let Some(java) = runtime_for(&state.runtimes, config.java_path.as_deref()) else {
        return "The chosen runtime is not known, so the command line is absent.".to_string();
    };
    let scratch = state.paths.cache_dir.join("advanced-preview");
    if let Err(error) = std::fs::create_dir_all(&scratch) {
        return format!("The preview is not available: {error}");
    }
    let paths = Paths {
        config_dir: scratch,
        ..state.paths.clone()
    };
    if let Err(error) = config.save(&paths) {
        return format!("The preview is not available: {error}");
    }
    let request = LaunchRequest {
        jar: &client.jar,
        kind: ClientKind::RuneLite,
        credentials: None,
        java: Some(java.path.as_path()),
        template: config.runelite_launch_command.as_deref(),
        configure: false,
    };
    match plan(&paths, &request) {
        Ok(plan) => {
            let lines = vec![
                format!("Working directory: {}", plan.working_dir.display()),
                format!("Environment names: {}", env_names(&plan.env)),
                "Command line:".to_string(),
                command_line(&plan.program, &plan.args),
            ];
            lines.join("\n")
        }
        Err(error) => format!("The preview is not available: {}", describe(&error)),
    }
}

/// Reads every control into a config.
fn collect(state: &AdvancedState) -> Config {
    let mut config = state.config.clone();
    config.java_path = state.java_choice.clone();
    config.runelite_process_name = optional_text(&state.process_name);
    config.runelite_home_kind = state.home_kind.clone();
    let tuning = &mut config.runelite_tuning;
    for check in &state.checks {
        check.field.write(tuning, is_on(&check.button));
    }
    tuning.heap_min = optional_text(&state.heap_min);
    tuning.heap_max = optional_text(&state.heap_max);
    tuning.stack_size = optional_text(&state.stack_size);
    tuning.garbage_collector = gc_choice(state.gc_popup.indexOfSelectedItem());
    tuning.application_name = optional_text(&state.application_name);
    tuning.extra_jvm_args = split_args(&state.extra_jvm_args);
    tuning.extra_app_args = split_args(&state.extra_app_args);
    config
}

/// Shows one control as text for the self check report.
fn control_title(view: &NSView) -> String {
    // `NSPopUpButton` is a subclass of `NSButton`, so it comes first.
    if let Some(popup) = view.downcast_ref::<NSPopUpButton>() {
        let selected = popup
            .titleOfSelectedItem()
            .map(|title| title.to_string())
            .unwrap_or_default();
        return format!("popup \"{selected}\" with {} items", popup.numberOfItems());
    }
    if let Some(button) = view.downcast_ref::<NSButton>() {
        // AppKit gives no reader for the button type, so every button reports its
        // state. A push button stays off, and a check box shows its real value.
        let state = if is_on(button) { "on" } else { "off" };
        return format!("button \"{}\" {state}", button.title());
    }
    if view.downcast_ref::<NSScrollView>().is_some() {
        return "scrollable text view".to_string();
    }
    if let Some(field) = view.downcast_ref::<NSTextField>() {
        let kind = if field.isEditable() { "field" } else { "label" };
        return format!("{kind} \"{}\"", field.stringValue());
    }
    "view".to_string()
}

/// The instance variables of the advanced window.
pub(crate) struct AdvancedIvars {
    paths: Paths,
    app: MainRef<AppDelegate>,
    state: Mutex<Option<AdvancedState>>,
    window: Mutex<Option<Retained<NSWindow>>>,
    /// Print the self check report after the discovery.
    report: bool,
}

define_class!(
    // SAFETY: The superclass NSObject has no subclassing requirements.
    // SAFETY: `AdvancedDelegate` does not implement `Drop`.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AdvancedIvars]
    pub(crate) struct AdvancedDelegate;

    // SAFETY: `NSObjectProtocol` has no safety requirements.
    unsafe impl NSObjectProtocol for AdvancedDelegate {}

    impl AdvancedDelegate {
        /// Reads the new Java choice.
        #[unsafe(method(javaChanged:))]
        fn java_changed(&self, _sender: Option<&AnyObject>) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            {
                let mut guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_mut() else {
                    return;
                };
                let index = state.java_popup.indexOfSelectedItem();
                if index == state.java_popup.numberOfItems() - 1 {
                    drop(guard);
                    self.choose_java(mtm);
                    return;
                }
                state.java_choice = selected_java(state);
            }
            self.refresh();
        }

        /// Reads a change of a launch option.
        #[unsafe(method(optionChanged:))]
        fn option_changed(&self, _sender: Option<&AnyObject>) {
            self.refresh();
        }

        /// Reads a change of the RuneLite home.
        #[unsafe(method(homeChanged:))]
        fn home_changed(&self, _sender: Option<&AnyObject>) {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let choose = {
                let guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_ref() else {
                    return;
                };
                state.home_popup.indexOfSelectedItem() == state.home_popup.numberOfItems() - 1
            };
            if choose {
                self.choose_home(mtm);
                return;
            }
            {
                let mut guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_mut() else {
                    return;
                };
                match state.home_popup.indexOfSelectedItem() {
                    0 => state.home_kind = RuneLiteHome::Isolated,
                    1 => state.home_kind = RuneLiteHome::System,
                    // Index 2 is the custom home. Its value is current already.
                    _ => {}
                }
            }
            self.set_window_status("The RuneLite home is changed.");
            self.refresh();
        }

        /// Examines the system home and asks the user about the copy.
        #[unsafe(method(importClicked:))]
        fn import_clicked(&self, _sender: Option<&AnyObject>) {
            let (paths, config) = {
                let guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_ref() else {
                    return;
                };
                (state.paths.clone(), collect(state))
            };
            if is_system_home(&config.runelite_home_kind) {
                self.set_window_status("The import needs a private home.");
                return;
            }
            self.set_window_status("The import is examined.");
            let me = MainRef::new(self);
            thread::spawn(move || {
                let result = import_plan(&paths, &config);
                MainCall::post(move || {
                    // SAFETY: The posted call runs on the main thread. The main
                    // window holds the delegate, so the object is alive.
                    let this = unsafe { me.get() };
                    this.import_planned(result);
                });
            });
        }

        /// Writes the config to the disk.
        #[unsafe(method(saveClicked:))]
        fn save_clicked(&self, _sender: Option<&AnyObject>) {
            let (config, result) = {
                let guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_ref() else {
                    return;
                };
                let config = collect(state);
                let result = config.save(&state.paths);
                (config, result)
            };
            let text = match result {
                Ok(()) => {
                    {
                        let mut guard = self.ivars().state.lock().unwrap();
                        if let Some(state) = guard.as_mut() {
                            state.config = config.clone();
                        }
                    }
                    let text = "The advanced options are saved.".to_string();
                    // SAFETY: The action runs on the main thread.
                    let app = unsafe { self.ivars().app.get() };
                    app.apply_config(&config, &text);
                    text
                }
                Err(error) => {
                    let text = format!("Could not save the advanced options: {error}");
                    // SAFETY: The action runs on the main thread.
                    let app = unsafe { self.ivars().app.get() };
                    app.set_status(&text);
                    text
                }
            };
            self.set_window_status(&text);
        }

        /// Reads the config from the disk again.
        #[unsafe(method(revertClicked:))]
        fn revert_clicked(&self, _sender: Option<&AnyObject>) {
            let config = {
                let guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_ref() else {
                    return;
                };
                Config::load(&state.paths)
            };
            self.show_config(&config);
            self.set_window_status("The config comes from the disk again.");
        }

        /// Puts the default tuned profile into the controls.
        #[unsafe(method(defaultsClicked:))]
        fn defaults_clicked(&self, _sender: Option<&AnyObject>) {
            let config = {
                let guard = self.ivars().state.lock().unwrap();
                let Some(state) = guard.as_ref() else {
                    return;
                };
                let mut config = collect(state);
                config.runelite_tuning = TuningConfig::default();
                config
            };
            self.show_config(&config);
            self.set_window_status("The tuned profile has the default values.");
        }
    }
);

impl AdvancedDelegate {
    /// Opens the advanced window.
    ///
    /// `report` prints the self check report after the Java discovery.
    pub(crate) fn open(
        mtm: MainThreadMarker,
        app: MainRef<AppDelegate>,
        paths: Paths,
        report: bool,
    ) -> Retained<AdvancedDelegate> {
        let this = Self::alloc(mtm).set_ivars(AdvancedIvars {
            paths,
            app,
            state: Mutex::new(None),
            window: Mutex::new(None),
            report,
        });
        let this: Retained<AdvancedDelegate> = unsafe { msg_send![super(this), init] };
        this.build(mtm);

        let me = MainRef::new(&*this);
        thread::spawn(move || {
            let runtimes = bolt_jdk::discover();
            MainCall::post(move || {
                // SAFETY: The posted call runs on the main thread. The main window
                // holds the delegate, so the object is alive.
                let this = unsafe { me.get() };
                this.runtimes_ready(runtimes);
            });
        });
        this
    }

    /// Builds the window and every control.
    fn build(&self, mtm: MainThreadMarker) {
        let target = Some(as_any(self));
        let config = {
            let paths = self.ivars().paths.clone();
            Config::load(&paths)
        };

        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, 780.0, 850.0),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(ns_string!("Advanced Options"));
        let content = window.contentView();
        let add = |view: &NSView| {
            if let Some(content) = &content {
                content.addSubview(view);
            }
        };

        let mut rows = Rows { bottom: 810.0 };

        let heading = make_label(mtm, "Java runtime", rect(20.0, rows.next(18.0), 740.0, 18.0));
        add(&heading);

        let java_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(20.0, rows.next(28.0), 740.0, 28.0),
            false,
        );
        unsafe {
            java_popup.setTarget(target);
            java_popup.setAction(Some(sel!(javaChanged:)));
        }
        add(&java_popup);

        let feature_field = make_label(
            mtm,
            "The chosen runtime is not known.",
            rect(20.0, rows.next(18.0), 740.0, 18.0),
        );
        add(&feature_field);

        rows.bottom -= 10.0;
        let heading = make_label(mtm, "Launch options", rect(20.0, rows.next(18.0), 740.0, 18.0));
        add(&heading);

        let enabled = make_check(
            mtm,
            target,
            sel!(optionChanged:),
            TuningField::Enabled.title(true),
            rect(20.0, rows.next(20.0), 320.0, 20.0),
        );
        add(&enabled);
        let mut checks = vec![Check {
            button: enabled,
            field: TuningField::Enabled,
        }];

        let row = rows.next(26.0);
        let heap_label = make_label(mtm, "Heap min", rect(20.0, row + 4.0, 66.0, 18.0));
        add(&heap_label);
        let heap_min = make_field(mtm, target, sel!(optionChanged:), rect(90.0, row, 70.0, 24.0));
        add(&heap_min);
        let heap_label = make_label(mtm, "Heap max", rect(176.0, row + 4.0, 66.0, 18.0));
        add(&heap_label);
        let heap_max = make_field(mtm, target, sel!(optionChanged:), rect(246.0, row, 70.0, 24.0));
        add(&heap_max);
        let stack_label = make_label(mtm, "Stack size", rect(328.0, row + 4.0, 66.0, 18.0));
        add(&stack_label);
        let stack_size =
            make_field(mtm, target, sel!(optionChanged:), rect(398.0, row, 66.0, 24.0));
        add(&stack_size);
        let gc_label = make_label(mtm, "Collector", rect(480.0, row + 4.0, 66.0, 18.0));
        add(&gc_label);
        let gc_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(548.0, row - 1.0, 190.0, 26.0),
            false,
        );
        for title in ["Default", "ZGC", "G1", "Parallel"] {
            gc_popup.addItemWithTitle(&NSString::from_str(title));
        }
        unsafe {
            gc_popup.setTarget(target);
            gc_popup.setAction(Some(sel!(optionChanged:)));
        }
        add(&gc_popup);

        let columns = [20.0, 400.0];
        let fields = [
            TuningField::CompactObjectHeaders,
            TuningField::StringDeduplication,
            TuningField::NativeAccess,
            TuningField::AotCache,
            TuningField::GcLog,
            TuningField::Java2dMetal,
            TuningField::LauncherNoJvm,
        ];
        for pair in fields.chunks(2) {
            let y = rows.next(22.0);
            for (column, field) in pair.iter().enumerate() {
                let x = columns[column];
                let button = make_check(
                    mtm,
                    target,
                    sel!(optionChanged:),
                    field.title(true),
                    rect(x, y, 350.0, 22.0),
                );
                add(&button);
                checks.push(Check { button, field: *field });
            }
        }

        let row = rows.next(24.0);
        let name_label = make_label(mtm, "Process name", rect(20.0, row + 4.0, 96.0, 18.0));
        add(&name_label);
        let process_name =
            make_field(mtm, target, sel!(optionChanged:), rect(120.0, row, 240.0, 24.0));
        add(&process_name);
        let app_label = make_label(mtm, "Application name", rect(400.0, row + 4.0, 110.0, 18.0));
        add(&app_label);
        let application_name =
            make_field(mtm, target, sel!(optionChanged:), rect(515.0, row, 225.0, 24.0));
        add(&application_name);

        let row = rows.next(24.0);
        let jvm_label = make_label(mtm, "Extra JVM arguments", rect(20.0, row + 4.0, 140.0, 18.0));
        add(&jvm_label);
        let extra_jvm_args =
            make_field(mtm, target, sel!(optionChanged:), rect(165.0, row, 575.0, 24.0));
        add(&extra_jvm_args);

        let row = rows.next(24.0);
        let app_args_label = make_label(mtm, "Extra app arguments", rect(20.0, row + 4.0, 140.0, 18.0));
        add(&app_args_label);
        let extra_app_args =
            make_field(mtm, target, sel!(optionChanged:), rect(165.0, row, 575.0, 24.0));
        add(&extra_app_args);

        rows.bottom -= 10.0;
        let heading = make_label(mtm, "RuneLite data", rect(20.0, rows.next(18.0), 740.0, 18.0));
        add(&heading);

        let home_popup = NSPopUpButton::initWithFrame_pullsDown(
            NSPopUpButton::alloc(mtm),
            rect(20.0, rows.next(28.0), 740.0, 28.0),
            false,
        );
        unsafe {
            home_popup.setTarget(target);
            home_popup.setAction(Some(sel!(homeChanged:)));
        }
        add(&home_popup);

        let home_label = make_label(mtm, "", rect(20.0, rows.next(18.0), 740.0, 18.0));
        home_label.setSelectable(true);
        add(&home_label);

        let row = rows.next(32.0);
        let import_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Import from ~/.runelite…"),
                target,
                Some(sel!(importClicked:)),
                mtm,
            )
        };
        import_button.setFrame(rect(20.0, row, 260.0, 32.0));
        add(&import_button);
        let secrets_check = make_check(
            mtm,
            target,
            sel!(optionChanged:),
            "Include the login files",
            rect(300.0, row + 5.0, 300.0, 22.0),
        );
        add(&secrets_check);

        rows.bottom -= 10.0;
        let heading = make_label(mtm, "Preview", rect(20.0, rows.next(18.0), 740.0, 18.0));
        add(&heading);

        let scroll = NSScrollView::initWithFrame(NSScrollView::alloc(mtm), rect(20.0, rows.next(150.0), 740.0, 150.0));
        scroll.setHasVerticalScroller(true);
        let preview = make_text_view(rect(0.0, 0.0, 720.0, 150.0));
        scroll.setDocumentView(Some(&*preview));
        add(&scroll);

        let default_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Defaults"),
                target,
                Some(sel!(defaultsClicked:)),
                mtm,
            )
        };
        default_button.setFrame(rect(20.0, 24.0, 110.0, 32.0));
        add(&default_button);

        let revert_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Revert"),
                target,
                Some(sel!(revertClicked:)),
                mtm,
            )
        };
        revert_button.setFrame(rect(140.0, 24.0, 100.0, 32.0));
        add(&revert_button);

        let save_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Save"),
                target,
                Some(sel!(saveClicked:)),
                mtm,
            )
        };
        save_button.setFrame(rect(250.0, 24.0, 100.0, 32.0));
        add(&save_button);

        let status = make_label(mtm, "Ready.", rect(370.0, 31.0, 370.0, 18.0));
        add(&status);

        *self.ivars().state.lock().unwrap() = Some(AdvancedState {
            paths: self.ivars().paths.clone(),
            java_choice: config.java_path.clone(),
            config: config.clone(),
            runtimes: Vec::new(),
            java_popup,
            feature_field,
            checks,
            heap_min,
            heap_max,
            stack_size,
            gc_popup,
            process_name,
            application_name,
            extra_jvm_args,
            extra_app_args,
            preview,
            status,
            home_kind: config.runelite_home_kind.clone(),
            home_popup,
            home_label,
            import_button,
            secrets_check,
            importing: false,
        });
        *self.ivars().window.lock().unwrap() = Some(window);

        self.show_config(&config);
        let window = self.ivars().window.lock().unwrap();
        if let Some(window) = window.as_ref() {
            window.center();
            window.makeKeyAndOrderFront(None);
        }
    }

    /// Puts a config into every control.
    fn show_config(&self, config: &Config) {
        {
            let mut guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_mut() else {
                return;
            };
            state.config = config.clone();
            state.java_choice = config.java_path.clone();
            keep_chosen(&mut state.runtimes, config);
            fill_java_popup(state);
            state.home_kind = config.runelite_home_kind.clone();
            fill_home_popup(state);
            for check in &state.checks {
                set_on(&check.button, check.field.read(&config.runelite_tuning));
            }
            let tuning = &config.runelite_tuning;
            state.heap_min.setStringValue(&shown_text(&tuning.heap_min));
            state.heap_max.setStringValue(&shown_text(&tuning.heap_max));
            state.stack_size.setStringValue(&shown_text(&tuning.stack_size));
            state
                .gc_popup
                .selectItemAtIndex(gc_index(&tuning.garbage_collector));
            state
                .process_name
                .setStringValue(&shown_text(&config.runelite_process_name));
            state
                .application_name
                .setStringValue(&shown_text(&tuning.application_name));
            state
                .extra_jvm_args
                .setStringValue(&NSString::from_str(&tuning.extra_jvm_args.join(" ")));
            state
                .extra_app_args
                .setStringValue(&NSString::from_str(&tuning.extra_app_args.join(" ")));
        }
        self.refresh();
    }

    /// Reads every control and shows the result.
    fn refresh(&self) {
        let guard = self.ivars().state.lock().unwrap();
        let Some(state) = guard.as_ref() else {
            return;
        };
        let config = collect(state);
        let feature = runtime_for(&state.runtimes, config.java_path.as_deref())
            .and_then(|runtime| runtime.version.as_ref())
            .map(|version| version.feature);
        let suffix = feature.unwrap_or(0) < FEATURE_24;

        let tuning_on = config.runelite_tuning.enabled;
        for check in &state.checks {
            if check.field != TuningField::Enabled {
                check.button.setEnabled(tuning_on);
            }
            if check.field.needs_feature_24() {
                check.button.setTitle(&NSString::from_str(check.field.title(suffix)));
            }
        }
        for field in [
            &state.heap_min,
            &state.heap_max,
            &state.stack_size,
            &state.process_name,
            &state.application_name,
            &state.extra_jvm_args,
            &state.extra_app_args,
        ] {
            field.setEnabled(tuning_on);
        }
        state.gc_popup.setEnabled(tuning_on);
        state
            .feature_field
            .setStringValue(&NSString::from_str(&feature_text(feature)));
        let home = config.runelite_home(&state.paths);
        state
            .home_label
            .setStringValue(&NSString::from_str(&home.display().to_string()));
        state
            .import_button
            .setEnabled(!is_system_home(&state.home_kind) && !state.importing);
        let text = preview_text(state, &config, feature.unwrap_or(FALLBACK_FEATURE));
        set_text(&state.preview, &text);
    }

    fn set_window_status(&self, text: &str) {
        let guard = self.ivars().state.lock().unwrap();
        if let Some(state) = guard.as_ref() {
            state.status.setStringValue(&NSString::from_str(text));
        }
    }

    /// Opens a panel for a Java program and probes the answer.
    fn choose_java(&self, mtm: MainThreadMarker) {
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(true);
        panel.setCanChooseDirectories(false);
        panel.setAllowsMultipleSelection(false);
        panel.setMessage(Some(ns_string!("Choose the java program of the runtime.")));
        let response = panel.runModal();
        {
            let guard = self.ivars().state.lock().unwrap();
            if let Some(state) = guard.as_ref() {
                select_java_choice(state);
            }
        }
        if response != MODAL_OK {
            self.refresh();
            return;
        }
        let Some(path) = panel.URL().and_then(|url| url.path()).map(|path| PathBuf::from(path.to_string())) else {
            self.refresh();
            return;
        };
        self.set_window_status("The Java runtime is probed.");
        let me = MainRef::new(self);
        thread::spawn(move || {
            let runtime = bolt_jdk::probe(&path);
            MainCall::post(move || {
                // SAFETY: The posted call runs on the main thread. The main window
                // holds the delegate, so the object is alive.
                let this = unsafe { me.get() };
                this.java_chosen(runtime);
            });
        });
    }

    /// Adds a probed runtime to the list and selects it.
    fn java_chosen(&self, runtime: Option<JavaRuntime>) {
        let added = {
            let mut guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_mut() else {
                return;
            };
            match runtime {
                Some(runtime) => {
                    state.java_choice = Some(runtime.path.clone());
                    if !state
                        .runtimes
                        .iter()
                        .any(|item| item.path == runtime.path)
                    {
                        state.runtimes.push(runtime);
                    }
                    fill_java_popup(state);
                    true
                }
                None => false,
            }
        };
        if added {
            self.set_window_status("The runtime is added to the list.");
        } else {
            self.set_window_status("The file is not a Java runtime.");
        }
        self.refresh();
    }

    /// Opens a panel for a home directory and writes the answer.
    fn choose_home(&self, mtm: MainThreadMarker) {
        let panel = NSOpenPanel::openPanel(mtm);
        panel.setCanChooseFiles(false);
        panel.setCanChooseDirectories(true);
        panel.setAllowsMultipleSelection(false);
        panel.setMessage(Some(ns_string!(
            "Choose the home directory of the RuneLite client."
        )));
        let response = panel.runModal();
        let path = if response == MODAL_OK {
            panel
                .URL()
                .and_then(|url| url.path())
                .map(|path| PathBuf::from(path.to_string()))
        } else {
            None
        };
        let text = {
            let mut guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_mut() else {
                return;
            };
            match path {
                Some(path) => {
                    state.home_kind = RuneLiteHome::Custom(path);
                    fill_home_popup(state);
                    "The home directory is chosen."
                }
                None => {
                    fill_home_popup(state);
                    "The home directory is not changed."
                }
            }
        };
        self.set_window_status(text);
        self.refresh();
    }

    /// Shows the import plan and asks the user.
    fn import_planned(&self, result: Result<ImportPlan, CoreError>) {
        let plan = match result {
            Ok(plan) => plan,
            Err(error) => {
                let text = format!("The import is not possible: {}", describe(&error));
                self.set_window_status(&text);
                if let Some(mtm) = MainThreadMarker::new() {
                    show_alert(mtm, "Import from ~/.runelite", &text, &["OK"]);
                }
                return;
            }
        };
        let count = plan.entries.len();
        let bytes: u64 = plan.entries.iter().map(|entry| entry.bytes).sum();
        let skipped = plan.entries.iter().filter(|entry| entry.present).count();
        let body = format!(
            "The source holds {}. The whole size is {}. The import skips {} that are present.",
            entry_count(count),
            size_text(bytes),
            entry_count(skipped),
        );
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let answer = show_alert(
            mtm,
            "Import from ~/.runelite",
            &body,
            &["Import", "Import and replace", "Cancel"],
        );
        if answer == NSAlertFirstButtonReturn {
            self.start_import(plan, false);
        } else if answer == NSAlertSecondButtonReturn {
            self.start_import(plan, true);
        }
    }

    /// Copies the plan on a worker thread.
    fn start_import(&self, plan: ImportPlan, overwrite: bool) {
        let secrets = {
            let mut guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_mut() else {
                return;
            };
            state.importing = true;
            is_on(&state.secrets_check)
        };
        self.set_window_status("The import runs.");
        self.refresh();
        let me = MainRef::new(self);
        thread::spawn(move || {
            let mut progress = move |name: &str| {
                let text = format!("Importing {name}…");
                MainCall::post(move || {
                    // SAFETY: The posted call runs on the main thread. The main
                    // window holds the delegate, so the object is alive.
                    let this = unsafe { me.get() };
                    this.set_window_status(&text);
                });
            };
            let result = import_apply(&plan, overwrite, secrets, &mut progress);
            MainCall::post(move || {
                // SAFETY: The posted call runs on the main thread. The main
                // window holds the delegate, so the object is alive.
                let this = unsafe { me.get() };
                this.import_done(result);
            });
        });
    }

    /// Shows the result of the copy.
    fn import_done(&self, result: Result<usize, CoreError>) {
        {
            let mut guard = self.ivars().state.lock().unwrap();
            if let Some(state) = guard.as_mut() {
                state.importing = false;
            }
        }
        let text = match result {
            Ok(count) => format!("The import copied {}.", entry_count(count)),
            Err(error) => format!("The import failed: {}", describe(&error)),
        };
        self.set_window_status(&text);
        self.refresh();
    }

    /// Fills the Java list and prints the self check report.
    fn runtimes_ready(&self, mut runtimes: Vec<JavaRuntime>) {
        {
            let mut guard = self.ivars().state.lock().unwrap();
            let Some(state) = guard.as_mut() else {
                return;
            };
            keep_chosen(&mut runtimes, &state.config);
            state.runtimes = runtimes;
            fill_java_popup(state);
        }
        self.refresh();
        if self.ivars().report {
            if let Some(mtm) = MainThreadMarker::new() {
                self.report(mtm);
            }
        }
    }

    /// Prints the state of the advanced window and of every control.
    fn report(&self, _mtm: MainThreadMarker) {
        let guard = self.ivars().window.lock().unwrap();
        let Some(window) = guard.as_ref() else {
            println!("self-check: no advanced window");
            return;
        };
        let frame = window.frame();
        println!(
            "self-check: advanced window \"{}\" visible={} size={}x{}",
            window.title(),
            window.isVisible(),
            frame.size.width,
            frame.size.height,
        );
        if let Some(content) = window.contentView() {
            for view in content.subviews().iter() {
                let bounds = view.frame();
                let enabled = view
                    .downcast_ref::<NSControl>()
                    .map(|control| control.isEnabled())
                    .unwrap_or(true);
                println!(
                    "self-check: advanced control \"{}\" enabled={} at {}x{}",
                    control_title(&view),
                    enabled,
                    bounds.origin.x,
                    bounds.origin.y
                );
            }
        }
        drop(guard);
        let info = {
            let guard = self.ivars().state.lock().unwrap();
            guard.as_ref().map(|state| {
                (
                    state.feature_field.stringValue().to_string(),
                    text_of(&state.preview).lines().count(),
                    home_kind_name(&state.home_kind),
                    state.home_label.stringValue().to_string(),
                    state.import_button.isEnabled(),
                    is_on(&state.secrets_check),
                )
            })
        };
        match info {
            Some((feature, lines, kind, home, import, secrets)) => {
                println!("self-check: advanced feature \"{feature}\"");
                println!("self-check: advanced preview lines {lines}");
                println!("self-check: advanced home kind \"{kind}\" dir \"{home}\"");
                println!(
                    "self-check: advanced import button enabled={import} secrets={secrets}"
                );
            }
            None => println!("self-check: the advanced window has no state"),
        }
    }
}
