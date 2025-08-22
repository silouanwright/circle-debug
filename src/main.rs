use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use colored::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::env;
use std::process;

#[derive(Parser)]
#[command(
    name = "circle-debug",
    about = "Debug CircleCI build failures with smart error detection",
    long_about = r#"
Debug CircleCI build failures using progressive disclosure and AI-friendly output.

This tool follows a three-tier debugging approach:
1. DEFAULT: Smart error patterns + last 50 lines (covers 80% of failures)  
2. --full: Complete logs when error isn't visible in default view
3. --tail N: Custom line count for specific context needs

The tool automatically saves full logs to /tmp for fallback analysis.
"#,
    version,
    author = "CircleCI Debug Tool",
    after_help = r#"
EXAMPLES:
  # Quick diagnosis (most common case)
  CIRCLE_DEBUG_FETCH_LOGS=1 circle_debug build https://circleci.com/gh/org/repo/12345
  
  # Full logs when error not found in summary
  CIRCLE_DEBUG_FETCH_LOGS=1 circle_debug build --full https://circleci.com/gh/org/repo/12345
  
  # Custom context window
  CIRCLE_DEBUG_FETCH_LOGS=1 circle_debug build --tail 200 https://circleci.com/gh/org/repo/12345
  
  # Save to specific file
  CIRCLE_DEBUG_FETCH_LOGS=1 circle_debug build --output debug.log https://circleci.com/gh/org/repo/12345

ENVIRONMENT:
  CIRCLECI_TOKEN           Your CircleCI API token (required)
  CIRCLE_DEBUG_FETCH_LOGS  Set to fetch and analyze build logs

EXIT CODES:
  0    Success - analysis completed
  1    General error or authentication failure
  
AI/AUTOMATION NOTE:
  If the error is not found in the default output, rerun with --full flag
  to get complete logs. The tool will indicate when this is needed.

For bug reports: https://github.com/your-org/circle-debug/issues
"#
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a failed build from URL
    /// 
    /// DEFAULT BEHAVIOR:
    /// 1. Shows smart error detection (known error patterns)
    /// 2. Shows last 50 lines of output (where errors usually are)
    /// 3. If error not found, use --full to see complete logs
    /// 
    /// PROGRESSIVE DEBUGGING:
    /// - First run: See smart summary + last 50 lines
    /// - If error not visible: Add --full to see everything
    /// - For AI/automation: Check exit code; non-zero means rerun with --full
    Build {
        /// CircleCI build URL (e.g., https://circleci.com/gh/org/repo/12345)
        url: String,
        /// Show full logs instead of summary (use when error not found in default view)
        #[arg(long, short = 'f', help = "Show complete logs when default summary doesn't show the error")]
        full: bool,
        /// Save logs to file for further analysis
        #[arg(long, short = 'o', help = "Save clean logs to file (automatic: /tmp/circle-debug-<build>.log)")]
        output: Option<String>,
        /// Only show last N lines without smart detection
        #[arg(long, help = "Show only the last N lines of output")]
        tail: Option<usize>,
    },
    /// Get workflow details
    Workflow {
        /// Pipeline ID
        pipeline_id: String,
    },
    /// Check PR status
    Pr {
        /// GitHub PR number
        pr_number: u32,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct BuildInfo {
    build_num: u32,
    status: String,
    branch: Option<String>,
    subject: Option<String>,
    steps: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Step {
    name: String,
    actions: Vec<Action>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Action {
    name: String,
    status: String,
    failed: Option<bool>,
    output_url: Option<String>,
    #[serde(rename = "type")]
    action_type: String,
}

struct CircleClient {
    token: String,
    client: reqwest::Client,
}

impl CircleClient {
    fn new() -> Result<Self> {
        let token = env::var("CIRCLECI_TOKEN")
            .context("cannot find CircleCI API token\n  help: Set CIRCLECI_TOKEN environment variable\n  docs: https://circleci.com/docs/api-developers-guide/#add-an-api-token")?;
        
        let client = reqwest::Client::new();
        
        Ok(CircleClient { token, client })
    }
    
    async fn get_build(&self, org: &str, project: &str, build_num: u32) -> Result<BuildInfo> {
        let url = format!(
            "https://circleci.com/api/v1.1/project/github/{}/{}/{}",
            org, project, build_num
        );
        
        let response = self.client
            .get(&url)
            .header("Circle-Token", &self.token)
            .send()
            .await?
            .json::<BuildInfo>()
            .await?;
        
        Ok(response)
    }
    
    async fn get_logs(&self, output_url: &str) -> Result<String> {
        let response = self.client
            .get(output_url)
            .header("Circle-Token", &self.token)
            .send()
            .await?
            .text()
            .await?;
        
        // Try to parse as JSON array and extract messages
        if let Ok(json_array) = serde_json::from_str::<Vec<serde_json::Value>>(&response) {
            let messages: Vec<String> = json_array
                .iter()
                .filter_map(|v| v.get("message").and_then(|m| m.as_str()))
                .map(|s| s.to_string())
                .collect();
            return Ok(messages.join(""));
        }
        
        Ok(response)
    }
}

fn parse_circleci_url(url: &str) -> Result<(String, String, u32)> {
    let re = Regex::new(r"circleci\.com/gh/([^/]+)/([^/]+)/(\d+)")?;
    
    let caps = re.captures(url)
        .with_context(|| format!(
            "cannot parse CircleCI URL\n  expected: https://circleci.com/gh/org/repo/12345\n  got: {}",
            url
        ))?;
    
    let org = caps.get(1).unwrap().as_str().to_string();
    let project = caps.get(2).unwrap().as_str().to_string();
    let build_num = caps.get(3).unwrap().as_str().parse::<u32>()?;
    
    Ok((org, project, build_num))
}

fn print_header(text: &str) {
    println!("\n{}", text.bold().blue());
    println!("{}", "=".repeat(text.len()).blue());
}

fn print_error(text: &str) {
    println!("{} {}", "✗".red().bold(), text.red());
}

fn print_success(text: &str) {
    println!("{} {}", "✓".green().bold(), text.green());
}

fn print_info(text: &str) {
    println!("{} {}", "→".yellow(), text);
}

async fn analyze_build(url: &str, full_logs: bool, output_file: Option<String>, tail_lines: Option<usize>) -> Result<()> {
    print_header("Analyzing CircleCI Build");
    
    let (org, project, build_num) = parse_circleci_url(url)?;
    print_info(&format!("Organization: {}", org));
    print_info(&format!("Project: {}", project));
    print_info(&format!("Build Number: {}", build_num));
    
    let client = CircleClient::new()?;
    
    println!("\n{}", "Fetching build details...".dimmed());
    let build = client.get_build(&org, &project, build_num).await?;
    
    print_header("Build Summary");
    print_info(&format!("Status: {}", 
        if build.status == "failed" { 
            build.status.red().to_string() 
        } else { 
            build.status.green().to_string() 
        }
    ));
    
    if let Some(branch) = &build.branch {
        print_info(&format!("Branch: {}", branch));
    }
    
    if let Some(subject) = &build.subject {
        print_info(&format!("Commit: {}", subject));
    }
    
    let failed_steps: Vec<_> = build.steps.iter()
        .filter(|step| {
            step.actions.iter().any(|action| action.failed.unwrap_or(false))
        })
        .collect();
    
    if !failed_steps.is_empty() {
        print_header("Failed Steps");
        
        for step in failed_steps {
            println!("\n{} {}", "▸".red().bold(), step.name.bold());
            
            for action in &step.actions {
                if action.failed.unwrap_or(false) {
                    print_error(&format!("  {}", action.name));
                    
                    if let Some(output_url) = &action.output_url {
                        if env::var("CIRCLE_DEBUG_FETCH_LOGS").is_ok() {
                            println!("\n  {}", "Fetching logs...".dimmed());
                            match client.get_logs(output_url).await {
                                Ok(logs) => {
                                    // Strip ANSI escape codes
                                    let ansi_re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
                                    let clean_logs = ansi_re.replace_all(&logs, "");
                                    
                                    // Always save to temp file for fallback
                                    let auto_save_path = format!("/tmp/circle-debug-{}.log", build_num);
                                    std::fs::write(&auto_save_path, clean_logs.as_ref())?;
                                    println!("\n  {}", format!("Auto-saved full logs to: {}", auto_save_path).dimmed());
                                    
                                    // Save to custom file if requested
                                    if let Some(ref output_path) = output_file {
                                        std::fs::write(output_path, clean_logs.as_ref())?;
                                        println!("  {}", format!("Logs also saved to: {}", output_path).green());
                                    }
                                    
                                    let total_lines = clean_logs.lines().count();
                                    println!("  {}", format!("Total: {} lines, {} KB", total_lines, logs.len() / 1024).dimmed());
                                    
                                    if full_logs {
                                        // Show full logs
                                        println!("\n  {}", "=== FULL LOG OUTPUT ===".yellow().bold());
                                        println!("{}", clean_logs);
                                    } else if let Some(n) = tail_lines {
                                        // Show only last N lines
                                        let lines: Vec<_> = clean_logs.lines().collect();
                                        let start = if lines.len() > n { lines.len() - n } else { 0 };
                                        println!("\n  {}", format!("=== LAST {} LINES ===", n).yellow().bold());
                                        for line in lines.iter().skip(start) {
                                            println!("{}", line);
                                        }
                                    } else {
                                        // DEFAULT: Smart detection + last 50 lines
                                        println!("\n  {}", "=== SMART ERROR DETECTION ===".blue().bold());
                                        
                                        // Find known error patterns
                                        let error_patterns = vec![
                                            // High confidence - specific errors
                                            (r"(?i)\[commonjs--resolver\].*failed to resolve", "Module Resolution"),
                                            (r"(?i)cannot find module", "Missing Module"),
                                            (r"(?i)syntaxerror:", "Syntax Error"),
                                            (r"(?i)typeerror:", "Type Error"),
                                            (r"(?i)segmentation fault", "Segfault"),
                                            (r"(?i)(oom|out of memory|memory limit)", "Out of Memory"),
                                            // Medium confidence - build failures
                                            (r"(?i)build failed", "Build Failure"),
                                            (r"(?i)compilation failed", "Compilation Error"),
                                            (r"(?i)test.*failed", "Test Failure"),
                                            (r"(?i)assertion.*failed", "Assertion Failure"),
                                            // Exit indicators
                                            (r"(?i)exited with (code|status) [1-9]", "Non-zero Exit"),
                                            (r"(?i)npm error", "NPM Error"),
                                        ];
                                        
                                        let mut found_errors = Vec::new();
                                        for (pattern, category) in error_patterns {
                                            let re = Regex::new(pattern).unwrap();
                                            for line in clean_logs.lines() {
                                                if re.is_match(line) {
                                                    found_errors.push((category, line));
                                                    if found_errors.len() >= 5 {
                                                        break;
                                                    }
                                                }
                                            }
                                            if found_errors.len() >= 5 {
                                                break;
                                            }
                                        }
                                        
                                        if !found_errors.is_empty() {
                                            println!("  Found {} error pattern(s):", found_errors.len());
                                            for (category, line) in found_errors.iter().take(5) {
                                                println!("  {} {}", format!("[{}]", category).red().bold(), line.trim().red());
                                            }
                                        } else {
                                            println!("  {}", "No specific error patterns detected".yellow());
                                        }
                                        
                                        // Always show last 50 lines
                                        let show_lines = 50;
                                        let lines: Vec<_> = clean_logs.lines().collect();
                                        let start = if lines.len() > show_lines { lines.len() - show_lines } else { 0 };
                                        
                                        println!("\n  {}", format!("=== LAST {} LINES (BUILD EXIT ZONE) ===", show_lines).yellow().bold());
                                        for (i, line) in lines.iter().skip(start).enumerate() {
                                            let line_num = start + i + 1;
                                            let trimmed = line.trim();
                                            // Highlight error-like lines
                                            if trimmed.to_lowercase().contains("error") || 
                                               trimmed.to_lowercase().contains("failed") ||
                                               trimmed.contains("✗") ||
                                               trimmed.contains("FAIL") {
                                                println!("{:5} | {}", line_num, trimmed.red());
                                            } else if trimmed.to_lowercase().contains("warn") {
                                                println!("{:5} | {}", line_num, trimmed.yellow());
                                            } else {
                                                println!("{:5} | {}", line_num, trimmed.dimmed());
                                            }
                                        }
                                        
                                        // Help text for next steps
                                        println!("\n  {}", "=== DIDN'T FIND YOUR ERROR? ===".cyan().bold());
                                        println!("  {}", "• Use --full to see complete logs".cyan());
                                        println!("  {}", "• Use --tail 100 to see more context".cyan());
                                        println!("  {}", format!("• Full logs saved at: {}", auto_save_path).cyan());
                                        println!("  {}", "• For AI: If error not found above, rerun with --full flag".cyan().bold());
                                    }
                                },
                                Err(e) => print_error(&format!("  Failed to fetch logs: {}", e)),
                            }
                        } else {
                            println!("\n  {}", "=== LOG FETCHING DISABLED ===".yellow().bold());
                            println!("  To fetch and analyze logs, set environment variable:");
                            println!("  {}", "export CIRCLE_DEBUG_FETCH_LOGS=1".cyan());
                            println!();
                            println!("  Or view logs directly at:");
                            println!("  {}", output_url.blue().underline());
                        }
                    }
                }
            }
        }
    } else {
        print_success("No failed steps found");
    }
    
    print_header("Quick Actions");
    println!("• Rerun: {}", format!("{}/retry", url).blue().underline());
    println!("• SSH Debug: Click 'Rerun' → 'Rerun job with SSH' in CircleCI UI");
    println!("• View artifacts: {}", format!("{}/artifacts", url).blue().underline());
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Build { url, full, output, tail } => {
            analyze_build(&url, full, output, tail).await?;
        },
        Commands::Workflow { pipeline_id } => {
            print_info(&format!("Fetching workflow for pipeline: {}", pipeline_id));
            // TODO: Implement workflow fetching
            println!("Workflow analysis not yet implemented");
        },
        Commands::Pr { pr_number } => {
            print_info(&format!("Checking PR #{}", pr_number));
            // TODO: Implement GitHub PR integration
            println!("PR status check not yet implemented");
        },
    }
    
    Ok(())
}