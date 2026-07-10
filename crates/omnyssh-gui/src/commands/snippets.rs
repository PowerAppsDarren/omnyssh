//! Snippet CRUD + execution (tech-gui.md §4.2). CRUD round-trips through the shared
//! `snippets.toml` (`load_snippets`/`save_snippets`); execute resolves the snippet +
//! hosts, substitutes only declared params, then runs the command on each host over a
//! fresh SSH session and emits a `snippet-result` per host (§4.3).

use std::collections::{HashMap, HashSet};

use tauri::{AppHandle, State};
use tauri_specta::Event;

use omnyssh_core::config::snippets::{load_snippets, save_snippets, Snippet};
use omnyssh_core::ssh::client::Host;
use omnyssh_core::ssh::session::SshSession;

use crate::dto::SnippetDto;
use crate::error::CommandError;
use crate::events;
use crate::state::GuiState;

/// List saved snippets from the shared `snippets.toml` (tech-gui.md §4.2).
#[tauri::command]
#[specta::specta]
pub async fn list_snippets() -> Result<Vec<SnippetDto>, CommandError> {
    let snippets = load().await?;
    Ok(snippets.iter().map(SnippetDto::from).collect())
}

/// Upsert a snippet by name and persist the whole list (tech-gui.md §4.2). A new
/// name appends; an existing name is replaced in place (an in-place edit). A rename
/// is a frontend delete-old + save-new, since the contract keys snippets on `name`.
#[tauri::command]
#[specta::specta]
pub async fn save_snippet(snippet: SnippetDto) -> Result<(), CommandError> {
    persist(move |snippets| {
        let incoming: Snippet = snippet.into();
        match snippets.iter_mut().find(|s| s.name == incoming.name) {
            Some(slot) => *slot = incoming,
            None => snippets.push(incoming),
        }
    })
    .await
}

/// Delete the snippet named `name` and persist (tech-gui.md §4.2). A missing name is
/// a no-op success — the desired end state (absent) already holds.
#[tauri::command]
#[specta::specta]
pub async fn delete_snippet(name: String) -> Result<(), CommandError> {
    persist(move |snippets| snippets.retain(|s| s.name != name)).await
}

/// Execute a snippet on one or more hosts (tech-gui.md §4.2). Substitutes the
/// provided values into declared placeholders, then fires one task per host that
/// runs the command over a fresh SSH session and emits a `snippet-result` (§4.3).
/// Fire-and-forget: results arrive as events, mirroring the core's own model.
#[tauri::command]
#[specta::specta]
pub async fn execute_snippet(
    app: AppHandle,
    state: State<'_, GuiState>,
    snippet_name: String,
    host_names: Vec<String>,
    params: HashMap<String, String>,
) -> Result<(), CommandError> {
    let snippet = load()
        .await?
        .into_iter()
        .find(|s| s.name == snippet_name)
        .ok_or_else(|| CommandError {
            message: format!("snippet '{snippet_name}' not found"),
        })?;

    let command = substitute_params(&snippet.command, snippet.params.as_deref(), &params);

    // Resolve full host records here; secret material stays backend-side (§3.4).
    let hosts = state.hosts_by_name(&host_names);
    if hosts.is_empty() {
        return Err(CommandError {
            message: "no matching hosts to execute on".to_string(),
        });
    }

    for host in hosts {
        let app = app.clone();
        let command = command.clone();
        let snippet_name = snippet_name.clone();
        tauri::async_runtime::spawn(async move {
            let (ok, output) = match run_on_host(&host, &command).await {
                Ok(stdout) => (true, stdout),
                Err(message) => (false, message),
            };
            let _ = events::SnippetResult {
                host_name: host.name,
                snippet_name,
                ok,
                output,
            }
            .emit(&app);
        });
    }
    Ok(())
}

/// Load the snippet list off the async worker (parsing is blocking I/O).
async fn load() -> Result<Vec<Snippet>, CommandError> {
    tauri::async_runtime::spawn_blocking(load_snippets)
        .await
        .map_err(|e| CommandError {
            message: format!("snippet load task failed: {e}"),
        })?
        .map_err(|e| CommandError {
            message: e.to_string(),
        })
}

/// Load the list, apply `mutate`, and write it back atomically off the async worker.
async fn persist(
    mutate: impl FnOnce(&mut Vec<Snippet>) + Send + 'static,
) -> Result<(), CommandError> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut snippets = load_snippets()?;
        mutate(&mut snippets);
        save_snippets(&snippets)
    })
    .await
    .map_err(|e| CommandError {
        message: format!("snippet save task failed: {e}"),
    })?
    .map_err(|e| CommandError {
        message: e.to_string(),
    })
}

/// Open a fresh SSH session, run `command`, disconnect, and return stdout or a
/// human-readable error. Mirrors the TUI's one-shot snippet execution path.
async fn run_on_host(host: &Host, command: &str) -> Result<String, String> {
    let session = SshSession::connect(host)
        .await
        .map_err(|e| format!("Connect failed: {e}"))?;
    let output = session
        .run_command(command)
        .await
        .map_err(|e| format!("Command failed: {e}"));
    session.disconnect().await;
    output
}

/// Substitute `{{name}}` placeholders in `command`, but only for names the snippet
/// declares and that a value was supplied for. Single-pass by design: an inserted
/// value is never re-scanned, so a value containing `{{other}}` cannot inject a
/// further substitution, and any undeclared `{{token}}` is left byte-for-byte
/// (tech-gui.md §7 Stage 2.2 acceptance: no injection of undeclared placeholders).
pub(crate) fn substitute_params(
    command: &str,
    declared: Option<&[String]>,
    values: &HashMap<String, String>,
) -> String {
    let Some(declared) = declared else {
        return command.to_string();
    };
    let declared: HashSet<&str> = declared.iter().map(String::as_str).collect();

    let mut out = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        match after.find("}}") {
            Some(close) => {
                let name = &after[..close];
                match values.get(name) {
                    Some(value) if declared.contains(name) => out.push_str(value),
                    // Undeclared, or declared-but-unprovided: keep the token literal.
                    _ => {
                        out.push_str("{{");
                        out.push_str(name);
                        out.push_str("}}");
                    }
                }
                rest = &after[close + 2..];
            }
            None => {
                // Unterminated `{{` — nothing more can substitute; emit verbatim.
                out.push_str("{{");
                out.push_str(after);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    // --- Example-based, pinning the core's substitution semantics ---------------

    #[test]
    fn substitutes_a_declared_param() {
        let out = substitute_params(
            "echo {{name}}",
            Some(&names(&["name"])),
            &map(&[("name", "world")]),
        );
        assert_eq!(out, "echo world");
    }

    #[test]
    fn substitutes_every_occurrence_of_a_declared_param() {
        let out = substitute_params("{{x}} {{x}}", Some(&names(&["x"])), &map(&[("x", "v")]));
        assert_eq!(out, "v v");
    }

    #[test]
    fn none_declared_returns_the_command_unchanged() {
        let out = substitute_params("echo {{x}}", None, &map(&[("x", "v")]));
        assert_eq!(out, "echo {{x}}");
    }

    #[test]
    fn an_empty_value_erases_the_placeholder() {
        let out = substitute_params("a{{p}}b", Some(&names(&["p"])), &map(&[("p", "")]));
        assert_eq!(out, "ab");
    }

    #[test]
    fn an_undeclared_placeholder_is_left_literal_even_with_a_value() {
        // A value keyed under an undeclared name is ignored — the token stays put.
        let out = substitute_params("run {{x}}", Some(&names(&["y"])), &map(&[("x", "boom")]));
        assert_eq!(out, "run {{x}}");
    }

    #[test]
    fn a_declared_param_with_no_value_is_left_literal() {
        let out = substitute_params(
            "{{a}} {{b}}",
            Some(&names(&["a", "b"])),
            &map(&[("a", "x")]),
        );
        assert_eq!(out, "x {{b}}");
    }

    #[test]
    fn a_substituted_value_is_not_re_expanded() {
        // The value of `a` looks like a placeholder for `b`, but it is inserted
        // literally and never re-scanned — the sentinel for `b` cannot leak.
        let out = substitute_params(
            "{{a}}",
            Some(&names(&["a", "b"])),
            &map(&[("a", "{{b}}"), ("b", "SENTINEL")]),
        );
        assert_eq!(out, "{{b}}");
    }

    #[test]
    fn a_single_brace_is_untouched() {
        let out = substitute_params("{name}", Some(&names(&["name"])), &map(&[("name", "v")]));
        assert_eq!(out, "{name}");
    }

    #[test]
    fn a_value_is_inserted_verbatim_without_shell_escaping() {
        // Pins current behaviour (matches the core): substitution is textual, the
        // frontend/user is responsible for trusting param values.
        let out = substitute_params(
            "sh -c {{c}}",
            Some(&names(&["c"])),
            &map(&[("c", "; rm -rf /")]),
        );
        assert_eq!(out, "sh -c ; rm -rf /");
    }

    // --- Property-based (tech-gui.md §7 Stage 2.2 test obligation) ---------------

    /// A `{{name}}` token built from a value the format machinery can quote cleanly.
    fn placeholder(name: &str) -> String {
        format!("{{{{{}}}}}", name)
    }

    // Param-name alphabet with no brace delimiters, so a generated name can never be
    // confused with the `{{`/`}}` framing.
    fn name_strat() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z_][a-zA-Z0-9_]{0,7}").expect("valid regex")
    }

    // Value alphabet without braces, so a value can't accidentally spell a placeholder
    // (the injection case is covered explicitly below with a crafted value).
    fn plain_value() -> impl Strategy<Value = String> {
        proptest::string::string_regex("[a-zA-Z0-9 ;/._-]{0,24}").expect("valid regex")
    }

    proptest! {
        // A declared + provided placeholder is replaced with exactly its value.
        #[test]
        fn declared_param_is_replaced_verbatim(n in name_strat(), v in plain_value()) {
            let cmd = format!("x{}y", placeholder(&n));
            let out = substitute_params(&cmd, Some(std::slice::from_ref(&n)), &map(&[(&n, &v)]));
            prop_assert_eq!(out, format!("x{v}y"));
        }

        // Only declared params are ever substituted: an `{{undeclared}}` token — even
        // when the values map carries an entry for it — survives byte-for-byte.
        #[test]
        fn undeclared_placeholder_survives(
            undeclared in name_strat(),
            declared in prop::collection::vec(name_strat(), 0..4),
            extra in plain_value(),
        ) {
            prop_assume!(!declared.contains(&undeclared));
            let cmd = format!("pre {} post", placeholder(&undeclared));
            let values = map(&[(&undeclared, &extra)]);
            let out = substitute_params(&cmd, Some(&declared), &values);
            prop_assert_eq!(out, cmd);
        }

        // No injection: a declared value that itself spells `{{other}}` is inserted
        // literally and never re-expanded, so `other`'s (sentinel) value cannot leak.
        #[test]
        fn a_declared_value_is_never_re_expanded(a in name_strat(), b in name_strat(), sentinel in "[A-Z]{4,10}") {
            prop_assume!(a != b);
            let values = map(&[(&a, &placeholder(&b)), (&b, &sentinel)]);
            let out = substitute_params(&placeholder(&a), Some(&names(&[&a, &b])), &values);
            // The exact output pins it: `a`'s value `{{b}}` lands verbatim and `b` is
            // NOT expanded to `sentinel` — that equality IS the no-injection proof.
            prop_assert_eq!(out, placeholder(&b));
        }

        // With no declared params, the command is returned untouched whatever values
        // are supplied (nothing to substitute against).
        #[test]
        fn no_declared_params_is_identity(n in name_strat(), v in plain_value()) {
            let cmd = format!("run {} now", placeholder(&n));
            let out = substitute_params(&cmd, None, &map(&[(&n, &v)]));
            prop_assert_eq!(out, cmd);
        }
    }
}
