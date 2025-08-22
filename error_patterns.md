# CircleCI Error Detection Strategy

## Recommended Approach: Multi-Pass Analysis with Confidence Scoring

### 1. **Error Categories with Confidence Levels**

```rust
enum ErrorCategory {
    Critical(f32),     // OOM, segfault, system crash (0.9-1.0 confidence)
    BuildFailure(f32), // Compilation, syntax errors (0.8-0.9)
    TestFailure(f32),  // Test assertions, failures (0.7-0.9)
    Infrastructure(f32), // Network, Docker, timeout (0.6-0.8)
    Generic(f32),      // Generic error patterns (0.3-0.6)
}
```

### 2. **Pattern Hierarchy**

**Tier 1: Exit Points (Highest Confidence)**
- "Exited with code exit status 1"
- "command exited (1)"
- "Build failed"
- "npm error Lifecycle script"

**Tier 2: Specific Error Patterns**
- "[commonjs--resolver] Failed to resolve"
- "SyntaxError:"
- "TypeError:"
- "Cannot find module"
- "ENOENT:"
- "OOM"
- "killed"
- "timeout"

**Tier 3: Generic Indicators**
- Lines starting with "ERROR"
- Lines containing "failed" or "Failed"
- Stack traces (multiple lines with "at " prefix)

### 3. **Context Extraction Rules**

- **For exit points**: Show 20 lines before the exit
- **For specific errors**: Show 5 lines before, 10 after
- **For test failures**: Include full test output
- **For stack traces**: Include until next non-indented line

### 4. **Default Behavior Recommendation**

```rust
// Default: Show smart summary with confidence indicators
circle-debug build <url>

// Output format:
========================================
High Confidence Errors (90%+):
- [Build] Failed to resolve package "@stitch-fix/graphql-api-provider"
  Location: line 234 in Build storybooks step

Medium Confidence Patterns (60-89%):
- Multiple npm lifecycle script failures detected
- 8 packages failed to build

Build Exit Point:
- Command exited with status 1 at line 1405

Note: Use --full to see complete logs
========================================
```

### 5. **Implementation Strategy**

```rust
struct ErrorDetector {
    patterns: HashMap<ErrorCategory, Vec<CompiledRegex>>,
    context_rules: HashMap<ErrorCategory, ContextRule>,
}

impl ErrorDetector {
    fn analyze(&self, logs: &str) -> Vec<DetectedError> {
        let mut errors = Vec::new();
        
        // Pass 1: Find exit point (work backwards)
        let exit_point = self.find_exit_point(logs);
        
        // Pass 2: Extract high-confidence errors
        errors.extend(self.find_specific_errors(logs));
        
        // Pass 3: Add context around exit if no specific errors found
        if errors.is_empty() {
            errors.push(self.extract_exit_context(exit_point));
        }
        
        // Sort by confidence and line number
        errors.sort_by(|a, b| {
            b.confidence.partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        
        errors
    }
}
```

### 6. **User Experience Improvements**

1. **Always show confidence level** so users know when to check --full
2. **Include line numbers** for easy navigation to source
3. **Group similar errors** (e.g., "5 similar test failures")
4. **Highlight actionable errors** vs informational messages
5. **Save raw logs by default** to `/tmp/circle-debug-<build-id>.log`

### 7. **Escape Hatch**

Always provide:
- `--full`: Show complete unfiltered logs
- `--raw`: Output without any formatting
- `--json`: Machine-readable output
- Automatic fallback to showing last 100 lines if no patterns match

This approach balances being helpful (finding real errors) with being honest (showing confidence levels) and providing escape hatches when the smart detection fails.