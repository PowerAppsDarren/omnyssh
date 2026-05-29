//! Tests for the SSH_ASKPASS prompt handling.
//!
//! The askpass helper must answer genuine password prompts with the stored
//! password, but never feed the password to other prompts (e.g. a host-key
//! `yes/no` confirmation).

use omnyssh::ssh::askpass::response_for_prompt;

#[test]
fn answers_standard_password_prompt() {
    let prompt = "user@host's password:";
    assert_eq!(
        response_for_prompt(prompt, "s3cret"),
        Some("s3cret".to_string())
    );
}

#[test]
fn answers_capitalized_password_prompt() {
    assert_eq!(
        response_for_prompt("Password:", "s3cret"),
        Some("s3cret".to_string())
    );
}

#[test]
fn ignores_host_key_confirmation_prompt() {
    let prompt = "The authenticity of host 'example.com' can't be established.\n\
                  Are you sure you want to continue connecting (yes/no/[fingerprint])?";
    assert_eq!(response_for_prompt(prompt, "s3cret"), None);
}

#[test]
fn ignores_empty_prompt() {
    assert_eq!(response_for_prompt("", "s3cret"), None);
}
