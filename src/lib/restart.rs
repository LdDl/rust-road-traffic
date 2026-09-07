//! Restarting the app from the inside, without knowing what supervises it
use std::io;
use std::path::PathBuf;
use std::process::Command;

use tracing::{error, info};

use crate::lib::logging;
use crate::video_capture::kill_capture_subprocesses;

/// The executable and the arguments a restart runs, i.e. exactly what this
/// process was started with
pub fn restart_command() -> io::Result<(PathBuf, Vec<String>)> {
    Ok((std::env::current_exe()?, std::env::args().skip(1).collect()))
}

/// Replaces the running process with a fresh copy of itself: same executable,
/// same arguments, same working directory and environment.
///
/// The point of replacing rather than exiting is that the app does not have to
/// know what started it. The process id stays the same, so systemd sees no
/// exit and restarts nothing, a container whose entrypoint is this binary keeps
/// running with its PID 1 in place, and a plain terminal run comes back too.
/// Nothing is saved on the way out: writing the config is a separate request.
///
/// Returns only on failure, in which case the process keeps running as it was
pub fn restart_process() -> io::Error {
    let (exe, args) = match restart_command() {
        Ok(command) => command,
        Err(e) => return e,
    };
    info!(
        scope = logging::SCOPE_STARTUP,
        executable = %exe.display(),
        args = ?args,
        "Restarting"
    );
    // Nothing below runs a destructor, so the capture subprocess has to go first
    kill_capture_subprocesses();
    let error = replace_process(&exe, &args);
    error!(
        scope = logging::SCOPE_STARTUP,
        executable = %exe.display(),
        error = %error,
        "Can't restart, keeping the process as it is"
    );
    error
}

#[cfg(unix)]
fn replace_process(exe: &PathBuf, args: &[String]) -> io::Error {
    use std::os::unix::process::CommandExt;
    // exec() only ever returns an error: on success this image is gone
    Command::new(exe).args(args).exec()
}

#[cfg(not(unix))]
fn replace_process(_exe: &PathBuf, _args: &[String]) -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "restarting in place is only implemented for unix",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_command_is_this_process() {
        let (exe, args) = restart_command().unwrap();
        assert!(exe.is_file(), "{}", exe.display());
        // The test binary gets its own arguments, they just must not include argv[0]
        assert!(!args.iter().any(|arg| arg == exe.to_string_lossy().as_ref()));
    }
}
