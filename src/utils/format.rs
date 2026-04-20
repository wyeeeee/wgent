/// 从工具输入生成一行预览文本
pub fn tool_input_preview(name: &str, input: &serde_json::Value) -> String {
    match name {
        "bash" => input["command"].as_str().unwrap_or("").to_string(),
        "read" => {
            let path = input["path"].as_str().unwrap_or("");
            match (input.get("start_line"), input.get("end_line")) {
                (Some(s), Some(e)) => format!("{} ({}-{})", path, s, e),
                (Some(s), None) => format!("{} ({}-末尾)", path, s),
                (None, Some(e)) => format!("{} (1-{})", path, e),
                _ => path.to_string(),
            }
        }
        "write" => format!("{} ({} 行)", input["path"].as_str().unwrap_or(""),
            input["content"].as_str().map(|c| c.lines().count()).unwrap_or(0)),
        "edit" => format!("{} ({}-{})", input["path"].as_str().unwrap_or(""),
            input["start_line"], input["end_line"]),
        _ => input.to_string().chars().take(80).collect(),
    }
}
