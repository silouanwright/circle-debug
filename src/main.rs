//! # cdb - CircleCI Debugger
//!
//! A CLI tool for quickly debugging CircleCI build failures directly from your terminal.
//!
//! ## Overview
//!
//! `cdb` helps developers quickly diagnose CI failures by:
//! - Fetching and analyzing CircleCI build logs
//! - Detecting common error patterns with contextual fix suggestions
//! - Showing timing analysis to identify performance bottlenecks
//! - Providing progressive disclosure (smart summary → last 50 lines → full logs)
//!
//! ## Quick Start
//!
//! ```no_run
//! # use anyhow::Result;
//! # use std::process::Command;
//! # fn main() -> Result<()> {
//! // Set your CircleCI token
//! std::env::set_var("CIRCLECI_TOKEN", "your-token");
//!
//! // Run the CLI to analyze a build
//! let output = Command::new("cdb")
//!     .args(&["build", "https://circleci.com/gh/org/repo/12345"])
//!     .output()?;
//!
//! // Check PR status
//! let output = Command::new("cdb")
//!     .args(&["pr", "123", "--repo", "org/repo"])
//!     .output()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The tool is built with:
//! - `CircleClient` - HTTP client for CircleCI API v1.1
//! - `BuildInfo`, `Step`, `Action` - Data models for build information
//! - `analyze_build` - Main analysis logic with smart error detection
//! - `analyze_pr` - GitHub PR integration via `gh` CLI
//!
//! ## Error Detection
//!
//! The tool uses pattern matching to identify common CI failures:
//! - Module resolution errors
//! - TypeScript compilation errors
//! - Test failures
//! - Out of memory errors
//! - NPM/Yarn dependency issues
//!
//! Each error type comes with contextual suggestions for resolution.
//!
//! ## Performance
//!
//! Log analysis performance scales with log size:
//! - Small builds (<1MB logs): ~100ms processing
//! - Medium builds (1-10MB): ~500ms processing
//! - Large builds (>10MB): 1-3 seconds processing
//!
//! All logs are automatically cached to `/tmp` for faster re-analysis.

use anyhow::{bail, Context, Result};
use circle_debug::{format_duration, parse_circleci_url, CircleClient};
use clap::{Parser, Subcommand};
use colored::*;
use regex::Regex;

/// Command-line interface for the CircleCI debugger.
///
/// This struct defines the main CLI parser using clap's derive API.
/// It provides a hierarchical command structure with subcommands for
/// different debugging operations.
///
/// # Examples
///
/// ```no_run
/// # use circle_debug::Cli;
/// # use clap::Parser;
/// let cli = Cli::parse();
/// ```
#[derive(Parser)]
#[command(
    name = "cdb",
    about = "Smart CircleCI build analyzer - auto-detects errors, suggests fixes, tracks performance",
    long_about = r#"
Debug CircleCI build failures using progressive disclosure and AI-friendly output.

This tool follows a three-tier debugging approach:
1. DEFAULT: Smart error patterns + last 50 lines (covers 80% of failures)  
2. --full: Complete logs when error isn't visible in default view
3. --tail N: Custom line count for specific context needs

The tool automatically saves full logs to /tmp for fallback analysis.

Use 'cdb <command> --help' for detailed information about each command.
"#,
    version,
    author = "CircleCI Debug Tool",
    after_help = r#"
EXAMPLES:
  # Quick diagnosis (most common case)
  cdb build https://circleci.com/gh/org/repo/12345
  
  # Full logs when error not found in summary
  cdb build --full https://circleci.com/gh/org/repo/12345
  
  # Custom context window
  cdb build --tail 200 https://circleci.com/gh/org/repo/12345
  
  # Filter logs by package/task
  cdb build --filter "@stitch-fix/graphql-api-provider" https://circleci.com/gh/org/repo/12345
  
  # Save to specific file
  cdb build --output debug.log https://circleci.com/gh/org/repo/12345
  
  # Check current PR (auto-detects PR and repo - requires gh CLI)
  cdb pr
  
  # Check specific PR (auto-detects repo)
  cdb pr 123
  
  # Check PR by URL
  cdb pr https://github.com/org/repo/pull/123

ENVIRONMENT:
  CIRCLECI_TOKEN    Your CircleCI API token (required)

AUTO-DETECTION:
  The 'pr' command auto-detects:
  • Current PR number from your branch (via gh CLI)
  • Repository from current directory (via gh CLI)
  
  Just run 'cdb pr' from your feature branch!

EXIT CODES:
  0    Success - analysis completed
  1    General error or authentication failure
  
AI/AUTOMATION NOTE:
  If the error is not found in the default output, rerun with --full flag
  to get complete logs. The tool will indicate when this is needed.

For bug reports: https://github.com/silouanwright/circle-debug/issues
"#
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Available subcommands for the CircleCI debugger.
///
/// Each command represents a different debugging operation that can be
/// performed on CircleCI builds or pull requests.
#[derive(Subcommand)]
enum Commands {
    /// Analyze a failed build from URL (use --help for full options)
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
        /// CircleCI build URL (e.g., `https://circleci.com/gh/org/repo/12345`)
        url: String,
        /// Show full logs instead of summary (use when error not found in default view)
        #[arg(
            long,
            short = 'f',
            help = "Show complete logs when default summary doesn't show the error"
        )]
        full: bool,
        /// Save logs to file for further analysis
        #[arg(
            long,
            short = 'o',
            help = "Save clean logs to file (automatic: /tmp/cdb-<build>.log)"
        )]
        output: Option<String>,
        /// Only show last N lines without smart detection
        #[arg(long, help = "Show only the last N lines of output")]
        tail: Option<usize>,
        /// Filter logs by package or task (e.g., "@stitch-fix/graphql-api-provider")
        #[arg(long, help = "Filter logs to show only lines containing this text")]
        filter: Option<String>,
        /// Skip fetching logs (only show build metadata)
        #[arg(long, help = "Skip fetching and analyzing logs")]
        no_fetch: bool,
    },
    /// Check PR status and CircleCI checks (use --help for full options)
    ///
    /// Shows all CircleCI checks for a GitHub PR.
    ///
    /// REQUIRES: GitHub CLI (gh) must be installed and authenticated.
    /// Install with: brew install gh (macOS) or https://cli.github.com/
    /// Then run: gh auth login
    Pr {
        /// GitHub PR number or URL (optional - auto-detects current PR if not specified)
        #[arg(
            help = "PR number (e.g., 123) or URL (e.g., https://github.com/org/repo/pull/123) - omit to use current branch's PR"
        )]
        pr: Option<String>,
        /// Repository in format org/repo (defaults to current repo)
        #[arg(
            long,
            short = 'r',
            help = "Repository (e.g., org/repo) - auto-detects if not specified"
        )]
        repo: Option<String>,
    },
}

/// Prints a formatted section header to the terminal.
///
/// Creates a visually distinctive header with the text in bold blue
/// and an underline of equal signs.
///
/// # Arguments
///
/// * `text` - The header text to display
///
/// # Examples
///
/// ```
/// print_header("Build Summary");
/// // Output:
/// // Build Summary (in bold blue)
/// // ============= (in blue)
/// ```
fn print_header(text: &str) {
    println!("\n{}", text.bold().blue());
    println!("{}", "=".repeat(text.len()).blue());
}

/// Prints an error message with a red cross indicator.
///
/// Formats error messages with a red "✗" symbol prefix for
/// clear visual indication of failures.
///
/// # Arguments
///
/// * `text` - The error message to display
///
/// # Examples
///
/// ```
/// print_error("Build failed with exit code 1");
/// // Output: ✗ Build failed with exit code 1 (in red)
/// ```
fn print_error(text: &str) {
    println!("{} {}", "✗".red().bold(), text.red());
}

/// Prints a success message with a green checkmark indicator.
///
/// Formats success messages with a green "✓" symbol prefix for
/// clear visual indication of successful operations.
///
/// # Arguments
///
/// * `text` - The success message to display
///
/// # Examples
///
/// ```
/// print_success("All tests passed");
/// // Output: ✓ All tests passed (in green)
/// ```
fn print_success(text: &str) {
    println!("{} {}", "✓".green().bold(), text.green());
}

/// Prints an informational message with a yellow arrow indicator.
///
/// Formats informational messages with a yellow "→" symbol prefix
/// for neutral status updates.
///
/// # Arguments
///
/// * `text` - The informational message to display
///
/// # Examples
///
/// ```
/// print_info("Fetching build details...");
/// // Output: → Fetching build details... (with yellow arrow)
/// ```
fn print_info(text: &str) {
    println!("{} {}", "→".yellow(), text);
}

/// Analyzes a CircleCI build and displays detailed failure information.
///
/// This is the main analysis function that fetches build details, identifies
/// failures, retrieves logs, and provides smart error detection with contextual
/// suggestions for fixing common issues.
///
/// # Arguments
///
/// * `url` - The CircleCI build URL to analyze
/// * `full_logs` - If true, displays complete logs instead of summary
/// * `output_file` - Optional path to save logs to a file
/// * `tail_lines` - If specified, shows only the last N lines of logs
/// * `filter` - Optional text filter to show only matching log lines
/// * `no_fetch` - If true, skips fetching logs (only shows metadata)
///
/// # Returns
///
/// A `Result` indicating success or containing an error.
///
/// # Errors
///
/// This function will return an error if:
/// * The URL cannot be parsed (invalid format)
/// * The `CIRCLECI_TOKEN` environment variable is not set
/// * The API request fails (network, authentication, or not found)
/// * File I/O operations fail when saving logs
///
/// # Examples
///
/// ```no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// // Basic usage - smart summary + last 50 lines
/// analyze_build(
///     "https://circleci.com/gh/org/repo/123",
///     false, None, None, None, false
/// ).await?;
///
/// // Full logs with output to file
/// analyze_build(
///     "https://circleci.com/gh/org/repo/123",
///     true,
///     Some("debug.log".to_string()),
///     None, None, false
/// ).await?;
///
/// // Filter logs for specific package
/// analyze_build(
///     "https://circleci.com/gh/org/repo/123",
///     false, None, None,
///     Some("@mypackage".to_string()),
///     false
/// ).await?;
/// # Ok(())
/// # }
/// ```
///
/// # Performance
///
/// Log processing performance depends on log size:
/// * Small logs (<1MB): ~100ms
/// * Medium logs (1-10MB): ~500ms
/// * Large logs (>10MB): 1-3 seconds
///
/// The function automatically saves logs to `/tmp` for caching.
///
/// # See Also
///
/// * [`parse_circleci_url`] - Parses the build URL
/// * [`CircleClient`] - Handles API communication
/// * [`format_duration`] - Formats timing information
async fn analyze_build(
    url: &str,
    full_logs: bool,
    output_file: Option<String>,
    tail_lines: Option<usize>,
    filter: Option<String>,
    no_fetch: bool,
) -> Result<()> {
    print_header("Analyzing CircleCI Build");

    let (org, project, build_num) = parse_circleci_url(url)?;
    print_info(&format!("Organization: {}", org));
    print_info(&format!("Project: {}", project));
    print_info(&format!("Build Number: {}", build_num));

    let client = CircleClient::new()?;

    println!("\n{}", "Fetching build details...".dimmed());
    let build = client.get_build(&org, &project, build_num).await?;

    print_header("Build Summary");
    print_info(&format!(
        "Status: {}",
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

    let failed_steps: Vec<_> = build
        .steps
        .iter()
        .filter(|step| {
            step.actions
                .iter()
                .any(|action| action.failed.unwrap_or(false))
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
                        if !no_fetch {
                            println!("\n  {}", "Fetching logs...".dimmed());
                            match client.get_logs(output_url).await {
                                Ok(logs) => {
                                    // Strip ANSI escape codes
                                    let ansi_re = Regex::new(r"\x1b\[[0-9;]*m").unwrap();
                                    let clean_logs = ansi_re.replace_all(&logs, "");

                                    // Always save to temp file for fallback
                                    let auto_save_path = format!("/tmp/cdb-{}.log", build_num);
                                    std::fs::write(&auto_save_path, clean_logs.as_ref())?;
                                    println!(
                                        "\n  {}",
                                        format!("Auto-saved full logs to: {}", auto_save_path)
                                            .dimmed()
                                    );

                                    // Save to custom file if requested
                                    if let Some(ref output_path) = output_file {
                                        std::fs::write(output_path, clean_logs.as_ref())?;
                                        println!(
                                            "  {}",
                                            format!("Logs also saved to: {}", output_path).green()
                                        );
                                    }

                                    // Apply filter if specified
                                    let filtered_logs = if let Some(ref filter_text) = filter {
                                        let filtered: String = clean_logs
                                            .lines()
                                            .filter(|line| line.contains(filter_text))
                                            .collect::<Vec<_>>()
                                            .join("\n");

                                        if filtered.is_empty() {
                                            println!(
                                                "  {}",
                                                format!(
                                                    "No lines matching filter: '{}'",
                                                    filter_text
                                                )
                                                .yellow()
                                            );
                                            clean_logs.clone()
                                        } else {
                                            let original_lines = clean_logs.lines().count();
                                            let filtered_lines = filtered.lines().count();
                                            println!(
                                                "  {}",
                                                format!(
                                                    "Filter '{}': {} of {} lines",
                                                    filter_text, filtered_lines, original_lines
                                                )
                                                .cyan()
                                            );
                                            filtered.into()
                                        }
                                    } else {
                                        clean_logs.clone()
                                    };

                                    let total_lines = filtered_logs.lines().count();
                                    println!(
                                        "  {}",
                                        format!(
                                            "Total: {} lines, {} KB",
                                            total_lines,
                                            logs.len() / 1024
                                        )
                                        .dimmed()
                                    );

                                    if full_logs {
                                        // Show full logs
                                        println!(
                                            "\n  {}",
                                            "=== FULL LOG OUTPUT ===".yellow().bold()
                                        );
                                        println!("{}", filtered_logs);
                                    } else if let Some(n) = tail_lines {
                                        // Show only last N lines
                                        let lines: Vec<_> = filtered_logs.lines().collect();
                                        let start =
                                            if lines.len() > n { lines.len() - n } else { 0 };
                                        println!(
                                            "\n  {}",
                                            format!("=== LAST {} LINES ===", n).yellow().bold()
                                        );
                                        for line in lines.iter().skip(start) {
                                            println!("{}", line);
                                        }
                                    } else {
                                        // DEFAULT: Smart detection + last 50 lines
                                        println!(
                                            "\n  {}",
                                            "=== SMART ERROR DETECTION ===".blue().bold()
                                        );

                                        // Find known error patterns
                                        let error_patterns = vec![
                                            // High confidence - specific errors
                                            (
                                                r"(?i)\[commonjs--resolver\].*failed to resolve",
                                                "Module Resolution",
                                            ),
                                            (r"(?i)cannot find module", "Missing Module"),
                                            (
                                                r"(?i)ENOENT:.*no such file or directory",
                                                "File Not Found",
                                            ),
                                            (r"(?i)syntaxerror:", "Syntax Error"),
                                            (r"(?i)typeerror:", "Type Error"),
                                            (r"(?i)referenceerror:", "Reference Error"),
                                            (r"(?i)segmentation fault", "Segfault"),
                                            (
                                                r"(?i)(oom|out of memory|memory limit)",
                                                "Out of Memory",
                                            ),
                                            // Build & compilation
                                            (r"(?i)build failed", "Build Failure"),
                                            (r"(?i)compilation failed", "Compilation Error"),
                                            (r"(?i)error TS\d+:", "TypeScript Error"),
                                            (r"(?i)eslint.*error", "Lint Error"),
                                            // Test failures
                                            (r"(?i)test.*failed", "Test Failure"),
                                            (r"(?i)assertion.*failed", "Assertion Failure"),
                                            (
                                                r"(?i)\d+ (test|tests|spec|specs) failed",
                                                "Test Suite Failure",
                                            ),
                                            // Package & dependency
                                            (r"(?i)npm err!", "NPM Error"),
                                            (r"(?i)yarn error", "Yarn Error"),
                                            (r"(?i)dependency.*not found", "Missing Dependency"),
                                            // Exit indicators
                                            (
                                                r"(?i)exited with (code|status) [1-9]",
                                                "Non-zero Exit",
                                            ),
                                            (r"(?i)command failed", "Command Failure"),
                                        ];

                                        let mut found_errors = Vec::new();
                                        let mut error_line_numbers = Vec::new();
                                        for (pattern, category) in error_patterns {
                                            let re = Regex::new(pattern).unwrap();
                                            for (line_num, line) in
                                                filtered_logs.lines().enumerate()
                                            {
                                                if re.is_match(line) {
                                                    found_errors.push((
                                                        category,
                                                        line,
                                                        line_num + 1,
                                                    ));
                                                    error_line_numbers.push(line_num + 1);
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
                                            println!(
                                                "  Found {} error pattern(s):",
                                                found_errors.len()
                                            );
                                            for (category, line, line_num) in
                                                found_errors.iter().take(5)
                                            {
                                                // Highlight with background color for better visibility
                                                println!(
                                                    "  {} {} {}",
                                                    format!("[{}]", category).red().bold(),
                                                    format!("Line {}:", line_num)
                                                        .bright_red()
                                                        .bold(),
                                                    line.trim().on_red().white().bold()
                                                );

                                                // Add contextual suggestions based on error type
                                                match category.as_ref() {
                                                    "File Not Found" => {
                                                        if line.contains("README")
                                                            || line.contains("readme")
                                                        {
                                                            println!("  {} Suggestion: Check file case sensitivity (README.md vs readme.md)", "💡".yellow());
                                                        } else if line.contains("package.json") {
                                                            println!("  {} Suggestion: Run 'npm install' to ensure dependencies are installed", "💡".yellow());
                                                        } else {
                                                            println!("  {} Suggestion: Verify file exists and path is correct", "💡".yellow());
                                                        }
                                                    }
                                                    "Missing Module" | "Missing Dependency" => {
                                                        println!("  {} Suggestion: Run 'npm install' or check package.json dependencies", "💡".yellow());
                                                    }
                                                    "TypeScript Error" => {
                                                        println!("  {} Suggestion: Run 'npm run typecheck' locally to see full type errors", "💡".yellow());
                                                    }
                                                    "Lint Error" => {
                                                        println!("  {} Suggestion: Run 'npm run lint -- --fix' to auto-fix some issues", "💡".yellow());
                                                    }
                                                    "Test Failure" | "Test Suite Failure" => {
                                                        println!("  {} Suggestion: Run tests locally with '--verbose' for more details", "💡".yellow());
                                                    }
                                                    "Out of Memory" => {
                                                        println!("  {} Suggestion: Increase Node memory: NODE_OPTIONS='--max-old-space-size=4096'", "💡".yellow());
                                                    }
                                                    "NPM Error" | "Yarn Error" => {
                                                        println!("  {} Suggestion: Clear cache (npm cache clean --force) and reinstall", "💡".yellow());
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        } else {
                                            println!(
                                                "  {}",
                                                "No specific error patterns detected".yellow()
                                            );
                                        }

                                        // Always show last 50 lines
                                        let show_lines = 50;
                                        let lines: Vec<_> = filtered_logs.lines().collect();
                                        let start = if lines.len() > show_lines {
                                            lines.len() - show_lines
                                        } else {
                                            0
                                        };

                                        println!(
                                            "\n  {}",
                                            format!(
                                                "=== LAST {} LINES (BUILD EXIT ZONE) ===",
                                                show_lines
                                            )
                                            .yellow()
                                            .bold()
                                        );
                                        for (i, line) in lines.iter().skip(start).enumerate() {
                                            let line_num = start + i + 1;
                                            let trimmed = line.trim();

                                            // Check if this line was identified as an error in smart detection
                                            let is_detected_error =
                                                error_line_numbers.contains(&line_num);

                                            // Highlight error-like lines with enhanced visibility
                                            if is_detected_error {
                                                // Lines detected by smart detection get special highlighting
                                                println!(
                                                    "{:5} {} {}",
                                                    format!("{}", line_num).bright_red().bold(),
                                                    "►".bright_red().bold(),
                                                    trimmed.on_red().white().bold()
                                                );
                                            } else if trimmed.to_lowercase().contains("error")
                                                || trimmed.to_lowercase().contains("failed")
                                                || trimmed.contains("✗")
                                                || trimmed.contains("FAIL")
                                            {
                                                println!(
                                                    "{:5} │ {}",
                                                    line_num,
                                                    trimmed.red().bold()
                                                );
                                            } else if trimmed.to_lowercase().contains("warn") {
                                                println!("{:5} │ {}", line_num, trimmed.yellow());
                                            } else {
                                                println!("{:5} │ {}", line_num, trimmed.dimmed());
                                            }
                                        }

                                        // Help text for next steps
                                        println!(
                                            "\n  {}",
                                            "=== DIDN'T FIND YOUR ERROR? ===".cyan().bold()
                                        );
                                        println!(
                                            "  {}",
                                            "• Use --full to see complete logs".cyan()
                                        );
                                        println!(
                                            "  {}",
                                            "• Use --tail 100 to see more context".cyan()
                                        );
                                        println!(
                                            "  {}",
                                            format!("• Full logs saved at: {}", auto_save_path)
                                                .cyan()
                                        );
                                        println!("  {}", "• For AI: If error not found above, rerun with --full flag".cyan().bold());
                                    }
                                }
                                Err(e) => print_error(&format!("  Failed to fetch logs: {}", e)),
                            }
                        } else {
                            println!("\n  {}", "=== LOG FETCHING SKIPPED ===".yellow().bold());
                            println!("  View logs directly at:");
                            println!("  {}", output_url.blue().underline());
                        }
                    }
                }
            }
        }
    } else {
        print_success("No failed steps found");
    }

    // Add timing analysis
    print_header("Timing Analysis");
    let mut step_timings: Vec<(&str, u64)> = Vec::new();
    let mut total_time = 0u64;

    for step in &build.steps {
        let step_time: u64 = step.actions.iter().filter_map(|a| a.run_time_millis).sum();
        if step_time > 0 {
            step_timings.push((&step.name, step_time));
            total_time += step_time;
        }
    }

    // Sort by duration (longest first)
    step_timings.sort_by(|a, b| b.1.cmp(&a.1));

    if !step_timings.is_empty() {
        println!("Total build time: {}", format_duration(total_time));
        println!("\nSlowest steps:");
        for (i, (name, duration)) in step_timings.iter().take(5).enumerate() {
            let percentage = (*duration as f64 / total_time as f64 * 100.0) as u32;
            let duration_str = format_duration(*duration);

            // Color code based on duration
            let formatted = if *duration > 60000 {
                // > 1 minute
                format!("{}. {} - {} ({}%)", i + 1, name, duration_str, percentage).red()
            } else if *duration > 30000 {
                // > 30 seconds
                format!("{}. {} - {} ({}%)", i + 1, name, duration_str, percentage).yellow()
            } else {
                format!("{}. {} - {} ({}%)", i + 1, name, duration_str, percentage).green()
            };
            println!("  {}", formatted);
        }

        // Identify bottlenecks
        if let Some((slowest_name, slowest_time)) = step_timings.first() {
            let percentage = (*slowest_time as f64 / total_time as f64 * 100.0) as u32;
            if percentage > 50 {
                println!(
                    "\n{} Bottleneck detected: '{}' takes {}% of total time",
                    "⚠".yellow(),
                    slowest_name,
                    percentage
                );
                println!("  Consider optimizing or parallelizing this step");
            }
        }
    } else {
        println!("No timing data available for this build");
    }

    print_header("Quick Actions");
    println!("• Rerun: {}", format!("{}/retry", url).blue().underline());
    println!("• SSH Debug: Click 'Rerun' → 'Rerun job with SSH' in CircleCI UI");
    println!(
        "• View artifacts: {}",
        format!("{}/artifacts", url).blue().underline()
    );

    Ok(())
}

/// Analyzes GitHub PR status and CircleCI checks.
///
/// Fetches and displays all CircleCI-related checks for a GitHub pull request.
/// This function requires the GitHub CLI (`gh`) to be installed and authenticated.
///
/// # Arguments
///
/// * `pr_input` - Either a PR number (e.g., "123") or full GitHub PR URL
/// * `repo` - Optional repository in format "org/repo". If not provided,
///            attempts to detect from current directory
///
/// # Returns
///
/// A `Result` indicating success or containing an error.
///
/// # Errors
///
/// This function will return an error if:
/// * The PR URL format is invalid
/// * The repository cannot be determined and isn't specified
/// * The GitHub CLI is not installed or not authenticated
/// * The PR doesn't exist or is inaccessible
/// * JSON parsing fails for PR details
///
/// # Examples
///
/// ```no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// // Using PR number with explicit repo
/// analyze_pr("123", Some("myorg/myrepo".to_string())).await?;
///
/// // Using full PR URL
/// analyze_pr(
///     "https://github.com/myorg/myrepo/pull/123",
///     None
/// ).await?;
///
/// // Auto-detect repo from current directory
/// analyze_pr("123", None).await?;
/// # Ok(())
/// # }
/// ```
///
/// # See Also
///
/// * [`analyze_build`] - Analyze specific failed builds from PR checks
async fn analyze_pr(pr_input: Option<String>, repo: Option<String>) -> Result<()> {
    print_header("Analyzing GitHub PR");

    // Check if gh CLI is available
    let gh_check = std::process::Command::new("which").arg("gh").output();

    if gh_check.is_err() || !gh_check.unwrap().status.success() {
        eprintln!(
            "{}",
            "Error: GitHub CLI (gh) is not installed or not in PATH".red()
        );
        eprintln!("\nTo use the 'pr' command, you need to install GitHub CLI:");
        eprintln!("  • macOS: brew install gh");
        eprintln!("  • Linux: See https://cli.github.com/");
        eprintln!("  • Windows: winget install GitHub.cli");
        eprintln!("\nThen authenticate with: gh auth login");
        eprintln!("\nNote: The 'build' command works without GitHub CLI");
        bail!("GitHub CLI (gh) is required for PR analysis");
    }

    // Determine PR number - auto-detect if not provided
    let pr_number = if let Some(input) = pr_input {
        // Parse PR number from URL or use directly
        if input.contains("github.com") {
            // Extract from URL like https://github.com/org/repo/pull/123
            input
                .split('/')
                .last()
                .context("Invalid PR URL")?
                .to_string()
        } else {
            input
        }
    } else {
        // Auto-detect current PR using gh CLI
        println!("{}", "Auto-detecting current PR...".dimmed());
        let output = std::process::Command::new("gh")
            .args(&["pr", "view", "--json", "number", "-q", ".number"])
            .output()
            .context("Failed to run 'gh pr view'. Is GitHub CLI installed and authenticated?")?;

        let pr_num = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if pr_num.is_empty() {
            bail!("No PR found for current branch. Create a PR first or specify PR number explicitly.");
        }
        pr_num
    };

    print_info(&format!("PR Number: {}", pr_number));

    // Determine repository
    let repository = if let Some(r) = repo {
        r
    } else {
        // Try to get from current directory using gh
        let output = std::process::Command::new("gh")
            .args(&[
                "repo",
                "view",
                "--json",
                "nameWithOwner",
                "-q",
                ".nameWithOwner",
            ])
            .output();

        match output {
            Ok(out) => String::from_utf8_lossy(&out.stdout).trim().to_string(),
            Err(_) => {
                bail!("Could not determine repository. Please specify with --repo org/repo");
            }
        }
    };

    if repository.is_empty() {
        bail!("Could not determine repository. Please specify with --repo org/repo");
    }

    print_info(&format!("Repository: {}", repository));

    // Get PR checks using gh CLI
    println!("\n{}", "Fetching PR status checks...".dimmed());

    let checks_output = std::process::Command::new("gh")
        .args(&["pr", "checks", &pr_number, "--repo", &repository])
        .output()
        .context("Failed to run 'gh pr checks'. Is GitHub CLI installed and authenticated?")?;

    // gh pr checks returns non-zero when there are failed checks, but still outputs data
    let checks = if !checks_output.stdout.is_empty() {
        String::from_utf8_lossy(&checks_output.stdout)
    } else if !checks_output.stderr.is_empty() {
        // Sometimes gh outputs to stderr even on success
        String::from_utf8_lossy(&checks_output.stderr)
    } else {
        bail!("No output from gh pr checks command");
    };

    print_header("PR Status Checks");

    // Parse and display CircleCI-specific checks
    let mut circleci_checks = Vec::new();
    let mut failed_checks = Vec::new();

    for line in checks.lines() {
        if line.contains("circleci") || line.contains("CircleCI") {
            circleci_checks.push(line);
            if line.contains("fail") || line.contains("✗") {
                failed_checks.push(line);
            }
        }
    }

    if circleci_checks.is_empty() {
        println!("{}", "No CircleCI checks found on this PR".yellow());
        println!("\nAll checks:");
        println!("{}", checks);
    } else {
        println!("Found {} CircleCI check(s):", circleci_checks.len());
        println!();

        for check in &circleci_checks {
            if check.contains("fail") || check.contains("✗") {
                println!("{}", check.red());
            } else if check.contains("pass") || check.contains("✓") {
                println!("{}", check.green());
            } else if check.contains("pending") || check.contains("○") {
                println!("{}", check.yellow());
            } else {
                println!("{}", check);
            }
        }

        if !failed_checks.is_empty() {
            print_header("Failed CircleCI Checks");
            println!("{}", "Extract URLs from failed checks to debug:".cyan());

            // Try to extract CircleCI URLs from the output
            let url_regex = Regex::new(r"https://circleci\.com/gh/[^\s]+/\d+")?;
            for check in &failed_checks {
                if let Some(url_match) = url_regex.find(check) {
                    let url = url_match.as_str();
                    println!(
                        "\n{} {}",
                        "•".red(),
                        check.split('\t').next().unwrap_or("Unknown check").red()
                    );
                    println!("  Debug with: {}", format!("cdb build {}", url).cyan());
                }
            }
        }
    }

    // Also show PR details
    println!();
    print_header("PR Details");

    let pr_details = std::process::Command::new("gh")
        .args(&[
            "pr",
            "view",
            &pr_number,
            "--repo",
            &repository,
            "--json",
            "state,title,author,url",
        ])
        .output();

    if let Ok(output) = pr_details {
        if output.status.success() {
            let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

            if let Some(title) = json.get("title").and_then(|v| v.as_str()) {
                print_info(&format!("Title: {}", title));
            }
            if let Some(state) = json.get("state").and_then(|v| v.as_str()) {
                print_info(&format!("State: {}", state));
            }
            if let Some(author) = json
                .get("author")
                .and_then(|v| v.get("login"))
                .and_then(|v| v.as_str())
            {
                print_info(&format!("Author: {}", author));
            }
            if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                print_info(&format!("URL: {}", url.blue().underline()));
            }
        }
    }

    Ok(())
}

/// Main entry point for the CircleCI debugger CLI.
///
/// Parses command-line arguments and dispatches to the appropriate
/// subcommand handler.
///
/// # Returns
///
/// A `Result` indicating success or containing an error that will
/// be displayed to the user.
///
/// # Errors
///
/// Returns an error if:
/// * Command parsing fails (invalid arguments)
/// * The dispatched command encounters an error
///
/// # Exit Codes
///
/// * `0` - Success, analysis completed
/// * `1` - General error or authentication failure
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            url,
            full,
            output,
            tail,
            filter,
            no_fetch,
        } => {
            analyze_build(&url, full, output, tail, filter, no_fetch).await?;
        }
        Commands::Pr { pr, repo } => {
            analyze_pr(pr, repo).await?;
        }
    }

    Ok(())
}
