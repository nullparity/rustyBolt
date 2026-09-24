//! The launch of a game client through a helper app bundle (macOS).
//!
//! macOS files a process under the app that spawns it. A client that
//! rustyBolt spawns shows the name "rustyBolt" in the menu bar, shares the
//! Dock icon of rustyBolt, and takes the Dock clicks for rustyBolt. Launch
//! Services files a process that it opens itself under the opened bundle. So
//! rustyBolt opens `Contents/Helpers/<client>.app` through `NSWorkspace`, and
//! the helper replaces itself with the client command.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSString, NSURL};

use crate::{ClientKind, CoreError, LaunchPlan};

/// The first argument that makes the helper run a client command.
pub const EXEC_CLIENT_COMMAND: &str = "__exec-client";

/// The directory below `Contents` where `macos/build-app.sh` puts the helpers.
pub const HELPERS_DIR: &str = "Helpers";

/// How long Launch Services can take to report the new process.
const OPEN_TIMEOUT: Duration = Duration::from_secs(30);

/// The Dock takes the name of the client from the folder name of its helper.
/// A binary outside an app bundle, such as a `cargo run` build, has no
/// helper. The caller then spawns the client directly.
pub(crate) fn helper_bundle(kind: ClientKind) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let contents = exe.parent()?.parent()?;
    let helper = contents
        .join(HELPERS_DIR)
        .join(format!("{}.app", kind.title()));
    helper
        .join("Contents/Info.plist")
        .is_file()
        .then_some(helper)
}

/// The arguments of the helper: the command, the working directory, the
/// program, and the arguments of the program.
fn helper_arguments(plan: &LaunchPlan) -> Vec<String> {
    let mut args = vec![
        EXEC_CLIENT_COMMAND.to_string(),
        plan.working_dir.to_string_lossy().into_owned(),
        plan.program.to_string_lossy().into_owned(),
    ];
    args.extend(plan.args.iter().cloned());
    args
}

/// Opens the helper through Launch Services and returns the pid of the client.
///
/// The login values travel in the environment of the new process. They never
/// appear in an argument list, which every user of the machine can read.
pub(crate) fn open_helper(helper: &Path, plan: &LaunchPlan) -> Result<u32, CoreError> {
    let url =
        NSURL::fileURLWithPath_isDirectory(&NSString::from_str(&helper.to_string_lossy()), true);

    let arguments: Vec<Retained<NSString>> = helper_arguments(plan)
        .iter()
        .map(|arg| NSString::from_str(arg))
        .collect();
    let names: Vec<Retained<NSString>> = plan
        .env
        .iter()
        .map(|(name, _)| NSString::from_str(name))
        .collect();
    let values: Vec<Retained<NSString>> = plan
        .env
        .iter()
        .map(|(_, value)| NSString::from_str(value))
        .collect();
    let name_refs: Vec<&NSString> = names.iter().map(|name| &**name).collect();

    let configuration = NSWorkspaceOpenConfiguration::configuration();
    configuration.setCreatesNewApplicationInstance(true);
    configuration.setAddsToRecentItems(false);
    configuration.setArguments(&NSArray::from_retained_slice(&arguments));
    configuration.setEnvironment(&NSDictionary::from_retained_objects(&name_refs, &values));

    let (sender, receiver) = mpsc::channel();
    let handler = RcBlock::new(move |app: *mut NSRunningApplication, error: *mut NSError| {
        // SAFETY: Launch Services passes a valid object or null for each.
        let result = match unsafe { app.as_ref() } {
            Some(app) if app.processIdentifier() > 0 => Ok(app.processIdentifier() as u32),
            _ => Err(unsafe { error.as_ref() }
                .map(|error| error.localizedDescription().to_string())
                .unwrap_or_else(|| "Launch Services returned no process".to_string())),
        };
        let _ = sender.send(result);
    });
    NSWorkspace::sharedWorkspace().openApplicationAtURL_configuration_completionHandler(
        &url,
        &configuration,
        Some(&handler),
    );

    match receiver.recv_timeout(OPEN_TIMEOUT) {
        Ok(Ok(pid)) => Ok(pid),
        Ok(Err(message)) => Err(CoreError::Io(std::io::Error::other(format!(
            "cannot open {}: {message}",
            helper.display()
        )))),
        Err(_) => Err(CoreError::Io(std::io::Error::other(format!(
            "{} did not start within {} seconds",
            helper.display(),
            OPEN_TIMEOUT.as_secs()
        )))),
    }
}

/// Runs the client command that [`open_helper`] passed to the helper.
///
/// `args` are the arguments after [`EXEC_CLIENT_COMMAND`]. On success the
/// function does not return, because the client replaces the helper.
pub fn exec_client(args: &[String]) -> CoreError {
    use std::os::unix::process::CommandExt;

    let [working_dir, program, rest @ ..] = args else {
        return CoreError::Io(std::io::Error::other(
            "the helper needs a working directory and a program",
        ));
    };
    let error = std::process::Command::new(program)
        .args(rest)
        .current_dir(working_dir)
        .exec();
    CoreError::Io(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_arguments_hold_the_command_the_directory_and_the_program() {
        let plan = LaunchPlan {
            program: PathBuf::from("/jdk/bin/RuneLite"),
            args: vec![
                "-Xmx1g".to_string(),
                "-jar".to_string(),
                "/a b/RuneLite.jar".to_string(),
            ],
            working_dir: PathBuf::from("/home/data"),
            env: vec![("JX_SESSION_ID".to_string(), "secret".to_string())],
        };

        assert_eq!(
            helper_arguments(&plan),
            [
                EXEC_CLIENT_COMMAND,
                "/home/data",
                "/jdk/bin/RuneLite",
                "-Xmx1g",
                "-jar",
                "/a b/RuneLite.jar",
            ]
        );
    }

    #[test]
    fn helper_arguments_never_hold_the_login_values() {
        let plan = LaunchPlan {
            program: PathBuf::from("/jdk/bin/java"),
            args: Vec::new(),
            working_dir: PathBuf::from("/"),
            env: vec![("JX_SESSION_ID".to_string(), "secret".to_string())],
        };

        assert!(!helper_arguments(&plan)
            .iter()
            .any(|arg| arg.contains("secret")));
    }

    #[test]
    fn exec_client_refuses_a_short_argument_list() {
        assert!(matches!(
            exec_client(&["/only-a-directory".to_string()]),
            CoreError::Io(_)
        ));
    }
}
