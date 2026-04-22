use anyhow::{anyhow, Result};
use async_trait::async_trait;
use ignore::WalkBuilder;
use regex::Regex;
use serde_json::{json, Value};

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
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a keyword across files in a directory. Matches both filenames and file contents, returning file paths, line numbers, and matching lines."
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

        let max_results = self.config.get().grep_max_results;
        let re = Regex::new(&format!("(?i){}", regex::escape(pattern)))?;
        let pattern_owned = pattern.to_string();

        tokio::task::spawn_blocking(move || grep_sync(&re, &search_dir, max_results))
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

fn grep_sync(re: &Regex, search_dir: &std::path::Path, max_results: usize) -> Result<(usize, Vec<String>)> {
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

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if re.is_match(name) {
                let rel = path.strip_prefix(search_dir).unwrap_or(path);
                results.push(format!("{} (filename match)", rel.display()));
            }
        }

        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if content.bytes().any(|b| b == 0) {
            continue;
        }

        let rel = path.strip_prefix(search_dir).unwrap_or(path);
        let mut file_matched = false;

        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                results.push(format!("{}:{} | {}", rel.display(), i + 1, line.trim_end()));
                file_matched = true;
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
