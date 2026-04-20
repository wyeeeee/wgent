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
        "在指定目录下全局搜索关键词。同时匹配文件名和文件内容，返回文件路径、行号和匹配行。"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "搜索关键词或正则表达式"
                },
                "path": {
                    "type": "string",
                    "description": "搜索目录路径（默认为工作目录）"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<String> {
        let pattern = input["pattern"]
            .as_str()
            .ok_or_else(|| anyhow!("缺少 pattern 参数"))?;

        if pattern.trim().is_empty() {
            return Err(anyhow!("pattern 不能为空"));
        }

        let search_dir = match input.get("path").and_then(|v| v.as_str()) {
            Some(p) => resolve_path(&ctx.working_dir, p)?,
            None => ctx.working_dir.clone(),
        };

        let re = Regex::new(&format!("(?i){}", regex::escape(pattern)))?;
        let max_results = self.config.get().grep_max_results;

        let mut results = Vec::new();
        let mut file_count = 0;

        for entry in WalkBuilder::new(&search_dir)
            .hidden(true)
            .git_ignore(true)
            .build()
        {
            if results.len() >= max_results {
                results.push(format!("... (已达到 {} 条上限)", max_results));
                break;
            }

            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            // 文件名匹配
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if re.is_match(name) {
                    let rel = path.strip_prefix(&search_dir).unwrap_or(path);
                    results.push(format!("{} (文件名匹配)", rel.display()));
                }
            }

            // 内容匹配（只搜文件，跳过目录和二进制）
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

            let rel = path.strip_prefix(&search_dir).unwrap_or(path);
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

        if results.is_empty() {
            return Ok(format!("未找到匹配 \"{}\" 的结果", pattern));
        }

        Ok(format!("搜索 \"{}\" ({} 个文件, {} 条结果):\n{}", pattern, file_count, results.len(), results.join("\n")))
    }
}
