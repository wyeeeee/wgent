/// Generate a one-line preview of tool input
pub fn tool_input_preview(name: &str, input: &serde_json::Value) -> String {
    match name {
        "Bash" => input["command"].as_str().unwrap_or("").to_string(),
        "Read" => {
            let path = input["path"].as_str().unwrap_or("");
            match (input.get("start_line"), input.get("end_line")) {
                (Some(s), Some(e)) => format!("{} ({}-{})", path, s, e),
                (Some(s), None) => format!("{} ({}-end)", path, s),
                (None, Some(e)) => format!("{} (1-{})", path, e),
                _ => path.to_string(),
            }
        }
        "Write" => format!("{} ({} lines)", input["path"].as_str().unwrap_or(""),
            input["content"].as_str().map(|c| c.lines().count()).unwrap_or(0)),
        "Edit" => {
            let path = input["path"].as_str().unwrap_or("");
            let old = input["old_string"].as_str().unwrap_or("");
            let preview = if old.len() > 40 {
                let bound = old.floor_char_boundary(40);
                format!("{}…", &old[..bound])
            } else if old.is_empty() {
                "(empty match)".to_string()
            } else {
                old.to_string()
            };
            format!("{} ({})", path, preview)
        }
        "MultiEdit" => {
            let path = input["path"].as_str().unwrap_or("");
            let count = input.get("edits").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
            format!("{} ({} edits)", path, count)
        }
        "WebFetch" => input["url"].as_str().unwrap_or("").to_string(),
        _ => input.to_string().chars().take(80).collect(),
    }
}
