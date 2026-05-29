//! SSH_ASKPASS integration for password-auth terminal sessions.
//!
//! The interactive terminal spawns the system `ssh` binary, which cannot read
//! the password stored in `hosts.toml` and therefore prompts for it on every
//! connection. To avoid that, the terminal points `SSH_ASKPASS` at our own
//! binary and passes the password through the child process environment (see
//! [`PASSWORD_ENV`]). When `ssh` needs a password it re-invokes us in askpass
//! mode and reads the answer from stdout — the password never touches disk or
//! the command line.

/// Environment variable carrying the stored password to our binary when it is
/// invoked as `ssh`'s askpass helper. Its presence signals askpass mode.
pub const PASSWORD_ENV: &str = "OMNY_ASKPASS_PASSWORD";

/// Decides what to print when invoked as `ssh`'s askpass program.
///
/// `prompt` is the text `ssh` passes as the first argument. Returns
/// `Some(password)` only for genuine password prompts; returns `None` for any
/// other prompt (e.g. a host-key `yes/no` confirmation) so the password is
/// never fed to a non-password question.
pub fn response_for_prompt(prompt: &str, password: &str) -> Option<String> {
    if prompt.to_lowercase().contains("password") {
        Some(password.to_string())
    } else {
        None
    }
}
