//! # CircleCI Debug Library
//!
//! Core functionality for debugging CircleCI builds.
//!
//! This library provides the underlying functionality used by the `cdb` CLI tool.
//! It can be used programmatically to analyze CircleCI builds and extract
//! failure information.
//!
//! ## Quick Start
//!
//! ```no_run
//! # use anyhow::Result;
//! # async fn example() -> Result<()> {
//! use circle_debug::{CircleClient, parse_circleci_url};
//!
//! // Parse a CircleCI URL
//! let (org, project, build_num) = parse_circleci_url(
//!     "https://circleci.com/gh/myorg/myrepo/12345"
//! )?;
//!
//! // Create a client and fetch build info
//! let client = CircleClient::new()?;
//! let build = client.get_build(&org, &project, build_num).await?;
//!
//! // Check build status
//! if build.status == "failed" {
//!     println!("Build failed!");
//!     for step in build.steps {
//!         for action in step.actions {
//!             if action.failed.unwrap_or(false) {
//!                 println!("Failed action: {}", action.name);
//!             }
//!         }
//!     }
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ## Features
//!
//! - **API Client**: [`CircleClient`] for interacting with CircleCI API
//! - **Data Models**: [`BuildInfo`], [`Step`], [`Action`] for build data
//! - **URL Parsing**: [`parse_circleci_url`] for extracting build information
//! - **Duration Formatting**: [`format_duration`] for human-readable time display
//!
//! ## Error Handling
//!
//! All functions return `anyhow::Result` for flexible error handling.
//! Errors include network failures, authentication issues, and parsing problems.

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub mod error;
pub use error::CircleDebugError;

/// CircleCI build information returned by the API.
///
/// Contains comprehensive information about a build including its status,
/// steps, actions, and timing data.
///
/// # See Also
///
/// * [`Step`] - Individual build steps
/// * [`CircleClient::get_build`] - Method to fetch this information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildInfo {
    /// The build number, unique within the project.
    pub build_num: u32,
    /// Build status (e.g., "success", "failed", "running").
    pub status: String,
    /// Git branch name for this build.
    pub branch: Option<String>,
    /// Commit message or subject line.
    pub subject: Option<String>,
    /// List of build steps executed in this build.
    pub steps: Vec<Step>,
}

impl BuildInfo {
    pub fn is_failed(&self) -> bool {
        self.status == "failed"
    }

    pub fn is_success(&self) -> bool {
        self.status == "success"
    }

    pub fn failed_actions(&self) -> impl Iterator<Item = &Action> {
        self.steps
            .iter()
            .flat_map(|step| step.actions.iter())
            .filter(|action| action.failed.unwrap_or(false))
    }
}

/// Represents a single step in a CircleCI build.
///
/// A step groups related actions that are executed sequentially.
///
/// # See Also
///
/// * [`Action`] - Individual actions within a step
/// * [`BuildInfo`] - Parent structure containing steps
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Step {
    /// The name of the step (e.g., "Run tests", "Build").
    pub name: String,
    /// List of actions executed within this step.
    pub actions: Vec<Action>,
}

impl Step {
    pub fn has_failures(&self) -> bool {
        self.actions.iter().any(|a| a.failed.unwrap_or(false))
    }
}

/// Represents an individual action within a CircleCI build step.
///
/// The smallest unit of execution in CircleCI with its own status and logs.
///
/// # See Also
///
/// * [`Step`] - Parent structure containing actions
/// * [`CircleClient::get_logs`] - Method to fetch action output
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Action {
    /// The name of the action (e.g., "npm test", "cargo build").
    pub name: String,
    /// Execution status (e.g., "success", "failed", "timedout").
    pub status: String,
    /// Whether this action failed.
    pub failed: Option<bool>,
    /// URL to fetch the full output logs for this action.
    pub output_url: Option<String>,
    /// The type of action (e.g., "test", "deploy").
    #[serde(rename = "type")]
    pub action_type: String,
    /// Execution time in milliseconds.
    pub run_time_millis: Option<u64>,
}

impl Action {
    pub fn is_failed(&self) -> bool {
        self.failed.unwrap_or(false) || self.status == "failed"
    }

    pub fn duration(&self) -> Duration {
        Duration::from_millis(self.run_time_millis.unwrap_or(0))
    }
}

/// HTTP client for interacting with the CircleCI API.
///
/// Handles authentication and provides methods to fetch build information
/// and logs from CircleCI.
///
/// # Authentication
///
/// Requires a CircleCI personal API token set via the `CIRCLECI_TOKEN`
/// environment variable.
///
/// # Examples
///
/// ```no_run
/// # use anyhow::Result;
/// # async fn example() -> Result<()> {
/// use circle_debug::CircleClient;
///
/// std::env::set_var("CIRCLECI_TOKEN", "your-token");
/// let client = CircleClient::new()?;
/// let build = client.get_build("org", "repo", 12345).await?;
/// # Ok(())
/// # }
/// ```
pub struct CircleClient {
    token: String,
    client: reqwest::Client,
}

impl CircleClient {
    /// Creates a new CircleCI API client.
    ///
    /// # Returns
    ///
    /// A `Result` containing the initialized client or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the `CIRCLECI_TOKEN` environment variable is not set.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # fn main() -> Result<()> {
    /// use circle_debug::CircleClient;
    ///
    /// std::env::set_var("CIRCLECI_TOKEN", "your-token");
    /// let client = CircleClient::new()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new() -> Result<Self> {
        Self::with_token(std::env::var("CIRCLECI_TOKEN").context(
            "cannot find CircleCI API token\n  help: Set CIRCLECI_TOKEN environment variable",
        )?)
    }

    pub fn with_token(token: impl Into<String>) -> Result<Self> {
        let token = token.into();
        if token.is_empty() {
            bail!("CircleCI token cannot be empty");
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(CircleClient { token, client })
    }

    /// Fetches build information from CircleCI.
    ///
    /// # Arguments
    ///
    /// * `org` - The GitHub organization name
    /// * `project` - The repository/project name
    /// * `build_num` - The CircleCI build number
    ///
    /// # Returns
    ///
    /// A `Result` containing the [`BuildInfo`] or an error.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the response cannot be parsed.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # async fn example() -> Result<()> {
    /// use circle_debug::CircleClient;
    ///
    /// let client = CircleClient::new()?;
    /// let build = client.get_build("myorg", "myrepo", 12345).await?;
    /// println!("Build status: {}", build.status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_build(&self, org: &str, project: &str, build_num: u32) -> Result<BuildInfo> {
        let url = format!(
            "https://circleci.com/api/v1.1/project/github/{}/{}/{}",
            org, project, build_num
        );

        let response = self
            .client
            .get(&url)
            .header("Circle-Token", &self.token)
            .send()
            .await
            .context("Failed to connect to CircleCI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response
                .text()
                .await
                .unwrap_or_else(|_| "<no response body>".to_string());
            bail!("CircleCI API returned error {}: {}", status, text);
        }

        let build_info = response
            .json::<BuildInfo>()
            .await
            .context("Failed to parse CircleCI response")?;

        Ok(build_info)
    }

    /// Fetches action logs from CircleCI.
    ///
    /// # Arguments
    ///
    /// * `output_url` - The URL to fetch logs from
    ///
    /// # Returns
    ///
    /// A `Result` containing the log output as a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// # async fn example() -> Result<()> {
    /// use circle_debug::CircleClient;
    ///
    /// let client = CircleClient::new()?;
    /// let build = client.get_build("org", "repo", 123).await?;
    ///
    /// for step in build.steps {
    ///     for action in step.actions {
    ///         if let Some(url) = action.output_url {
    ///             let logs = client.get_logs(&url).await?;
    ///             println!("Logs: {}", logs);
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn get_logs(&self, output_url: &str) -> Result<String> {
        let response = self
            .client
            .get(output_url)
            .header("Circle-Token", &self.token)
            .send()
            .await
            .context("Failed to fetch logs from CircleCI")?;

        if !response.status().is_success() {
            bail!("Failed to fetch logs: HTTP {}", response.status());
        }

        let text = response
            .text()
            .await
            .context("Failed to read log response")?;

        // Try to parse as JSON array and extract messages
        if let Ok(json_array) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
            let messages: Vec<String> = json_array
                .iter()
                .filter_map(|v| v.get("message").and_then(|m| m.as_str()))
                .map(|s| s.to_string())
                .collect();
            return Ok(messages.join(""));
        }

        Ok(text)
    }
}

/// Parses a CircleCI URL to extract organization, project, and build number.
///
/// # Arguments
///
/// * `url` - A CircleCI build URL
///
/// # Returns
///
/// A tuple of `(organization, project, build_number)`.
///
/// # Errors
///
/// Returns an error if the URL doesn't match the expected format.
///
/// # Examples
///
/// ```
/// # use anyhow::Result;
/// # fn main() -> Result<()> {
/// use circle_debug::parse_circleci_url;
///
/// let (org, proj, num) = parse_circleci_url(
///     "https://circleci.com/gh/myorg/myrepo/12345"
/// )?;
/// assert_eq!(org, "myorg");
/// assert_eq!(proj, "myrepo");
/// assert_eq!(num, 12345);
/// # Ok(())
/// # }
/// ```
pub fn parse_circleci_url(url: &str) -> Result<(String, String, u32)> {
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

/// Formats a duration from milliseconds to a human-readable string.
///
/// # Arguments
///
/// * `millis` - Duration in milliseconds
///
/// # Returns
///
/// A formatted string like "2m 30s" or "45s".
///
/// # Examples
///
/// ```
/// use circle_debug::format_duration;
///
/// assert_eq!(format_duration(0), "0s");
/// assert_eq!(format_duration(45000), "45s");
/// assert_eq!(format_duration(150000), "2m 30s");
/// ```
///
/// # Performance
///
/// O(1) time complexity with simple arithmetic operations.
pub fn format_duration(millis: u64) -> String {
    let seconds = millis / 1000;
    let minutes = seconds / 60;
    let remaining_seconds = seconds % 60;

    if minutes > 0 {
        format!("{}m {}s", minutes, remaining_seconds)
    } else {
        format!("{}s", seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_circleci_url() {
        let (org, proj, num) =
            parse_circleci_url("https://circleci.com/gh/myorg/myrepo/12345").unwrap();
        assert_eq!(org, "myorg");
        assert_eq!(proj, "myrepo");
        assert_eq!(num, 12345);
    }

    #[test]
    fn test_parse_circleci_url_variants() {
        let test_cases = vec![
            ("https://circleci.com/gh/org/repo/123", ("org", "repo", 123)),
            (
                "https://app.circleci.com/gh/org/repo/999",
                ("org", "repo", 999),
            ),
            (
                "http://circleci.com/gh/test/project/1",
                ("test", "project", 1),
            ),
        ];

        for (url, expected) in test_cases {
            let (org, proj, num) = parse_circleci_url(url).unwrap();
            assert_eq!(org, expected.0);
            assert_eq!(proj, expected.1);
            assert_eq!(num, expected.2);
        }
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(1000), "1s");
        assert_eq!(format_duration(45000), "45s");
        assert_eq!(format_duration(60000), "1m 0s");
        assert_eq!(format_duration(150000), "2m 30s");
        assert_eq!(format_duration(999), "0s");
        assert_eq!(format_duration(59999), "59s");
    }

    #[test]
    fn test_parse_circleci_url_invalid() {
        let invalid_urls = vec![
            "https://example.com/invalid",
            "not-a-url",
            "https://circleci.com/",
            "https://circleci.com/gh/",
            "https://circleci.com/gh/org/",
            "https://circleci.com/gh/org/repo/",
            "https://circleci.com/gh/org/repo/notanumber",
        ];

        for url in invalid_urls {
            let result = parse_circleci_url(url);
            assert!(result.is_err(), "URL should be invalid: {}", url);
        }
    }

    #[test]
    fn test_build_info_helpers() {
        let build = BuildInfo {
            build_num: 123,
            status: "failed".to_string(),
            branch: Some("main".to_string()),
            subject: Some("Test commit".to_string()),
            steps: vec![Step {
                name: "Test".to_string(),
                actions: vec![Action {
                    name: "Run tests".to_string(),
                    status: "failed".to_string(),
                    failed: Some(true),
                    output_url: Some("http://example.com/logs".to_string()),
                    action_type: "test".to_string(),
                    run_time_millis: Some(5000),
                }],
            }],
        };

        assert!(build.is_failed());
        assert!(!build.is_success());
        assert_eq!(build.failed_actions().count(), 1);
        assert!(build.steps[0].has_failures());
    }

    #[test]
    fn test_action_helpers() {
        let action = Action {
            name: "Test".to_string(),
            status: "success".to_string(),
            failed: Some(false),
            output_url: None,
            action_type: "test".to_string(),
            run_time_millis: Some(3000),
        };

        assert!(!action.is_failed());
        assert_eq!(action.duration(), Duration::from_millis(3000));

        let failed_action = Action {
            name: "Test".to_string(),
            status: "failed".to_string(),
            failed: Some(true),
            output_url: None,
            action_type: "test".to_string(),
            run_time_millis: None,
        };

        assert!(failed_action.is_failed());
        assert_eq!(failed_action.duration(), Duration::from_millis(0));
    }

    #[test]
    fn test_client_with_token() {
        let result = CircleClient::with_token("");
        assert!(result.is_err());

        let result = CircleClient::with_token("valid-token");
        assert!(result.is_ok());
    }
}

