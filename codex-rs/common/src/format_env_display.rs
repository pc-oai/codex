use std::collections::HashMap;

/// Format a combined environment display string from a map of explicit
/// key/value pairs and a list of passthrough env var names.
///
/// Examples:
/// - map={FOO=bar}, vars=[HOME, PATH] -> "FOO=bar, $HOME, $PATH"
/// - map=None/empty, vars=[HOME] -> "$HOME"
/// - both empty -> "-"
pub fn format_env_display(env: Option<&HashMap<String, String>>, env_vars: Vec<String>) -> String {
    let mut parts: Vec<String> = Vec::new();

    if let Some(map) = env {
        if !map.is_empty() {
            let mut pairs: Vec<_> = map.iter().collect();
            pairs.sort_by(|(a, _), (b, _)| a.cmp(b));
            parts.extend(pairs.into_iter().map(|(k, v)| format!("{k}={v}")));
        }
    }

    if !env_vars.is_empty() {
        let mut vars = env_vars.clone();
        vars.sort();
        parts.extend(vars.into_iter().map(|name| format!("${name}")));
    }

    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(", ")
    }
}
