use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::Regex;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};

use crate::config::Config;
use crate::tools::tool::{Tool, ToolContext};
use crate::utils::resolve_path;

pub struct GrepTool {
    config: Config,
}

impl GrepTool {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "Grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern across files in a directory. Supports regex, file type filtering (e.g. 'rs', 'py'), and respects .gitignore."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Search keyword or regex pattern"
                },
                "path": {
                    "type": "string",
                    "description": "Directory path to search (defaults to working directory)"
                },
                "file_type": {
                    "type": "string",
                    "description": "File extension filter, e.g. 'rs', 'py', 'ts' (optional, searches all files if omitted)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("Missing 'pattern' parameter"))?;

        if pattern.trim().is_empty() {
            return Err(anyhow!("Pattern cannot be empty"));
        }

        let search_dir = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => resolve_path(&ctx.working_dir, p)?,
            None => ctx.working_dir.clone(),
        };

        let file_type = input.get("file_type").and_then(|v| v.as_str()).map(|s| s.to_string());
        let max_results = self.config.get().grep_max_results;
        let re = Regex::new(&format!("(?i){}", regex::escape(pattern)))?;
        let pattern_owned = pattern.to_string();

        tokio::task::spawn_blocking(move || grep_sync(&re, &search_dir, max_results, file_type.as_deref()))
            .await
            .map_err(|e| anyhow!("Search task failed: {e}"))?
            .map(|(file_count, results)| {
                if results.is_empty() {
                    format!("No matches found for \"{}\"", pattern_owned)
                } else {
                    format!(
                        "Results for \"{}\" ({} files, {} matches):\n{}",
                        pattern_owned,
                        file_count,
                        results.len(),
                        results.join("\n")
                    )
                }
            })
    }
}

fn grep_sync(re: &Regex, search_dir: &std::path::Path, max_results: usize, file_type: Option<&str>) -> Result<(usize, Vec<String>)> {
    let mut results = Vec::new();
    let mut file_count = 0;

    for entry in WalkBuilder::new(search_dir)
        .hidden(true)
        .git_ignore(true)
        .build()
    {
        if results.len() >= max_results {
            results.push(format!("... (reached limit of {} results)", max_results));
            break;
        }

        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && re.is_match(name)
        {
            let rel = path.strip_prefix(search_dir).unwrap_or(path);
            results.push(format!("{} (filename match)", rel.display()));
        }

        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }

        if let Some(ext) = file_type {
            if path.extension().and_then(|e| e.to_str()) != Some(ext) {
                continue;
            }
        }

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut reader = BufReader::new(file);
        let mut buf = Vec::new();

        // Quick binary check on first chunk
        let bytes_read = reader.read_until(b'\n', &mut buf).unwrap_or(0);
        if bytes_read > 0 && buf.contains(&0) {
            continue;
        }

        let rel = path.strip_prefix(search_dir).unwrap_or(path);
        let mut file_matched = false;
        let mut line_num = 0;

        loop {
            line_num += 1;
            if line_num == 1 {
                // Process the first line we already read during binary check
                if let Ok(text) = std::str::from_utf8(&buf) {
                    let line = text.trim_end_matches(['\r', '\n']);
                    if re.is_match(line) {
                        results.push(format!("{}:{} | {}", rel.display(), line_num, line.trim_end()));
                        file_matched = true;
                    }
                }
            } else {
                buf.clear();
                match reader.read_until(b'\n', &mut buf) {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(text) = std::str::from_utf8(&buf) {
                            let line = text.trim_end_matches(['\r', '\n']);
                            if re.is_match(line) {
                                results.push(format!("{}:{} | {}", rel.display(), line_num, line.trim_end()));
                                file_matched = true;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }

            if results.len() >= max_results {
                break;
            }
        }

        if file_matched {
            file_count += 1;
        }
    }

    Ok((file_count, results))
}
