//! Context budget scanner.
//! Scans ~/.claude/ filesystem to estimate per-session token overhead.
//! No AI calls, pure filesystem analysis.

use std::path::Path;
use std::fs;
use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};

/// Rough bytes-to-tokens ratio (GPT/Claude tokenizers average ~4 bytes per token for English)
const BYTES_PER_TOKEN: f64 = 4.0;

fn estimate_tokens(bytes: u64) -> u64 {
    (bytes as f64 / BYTES_PER_TOKEN).ceil() as u64
}

#[derive(Debug, Clone)]
pub struct BudgetItem {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub tokens: u64,
    pub category: Category,
    pub loaded: Loaded,
    pub modified: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Category {
    ClaudeMd,
    Memory,
    Skill,
    Plugin,
    Hook,
    Settings,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Loaded {
    Always,    // loaded every message
    OnDemand,  // loaded when invoked
}

#[derive(Debug, Clone)]
pub struct BudgetReport {
    pub items: Vec<BudgetItem>,
    pub always_bytes: u64,
    pub always_tokens: u64,
    pub ondemand_bytes: u64,
    pub ondemand_tokens: u64,
    pub skill_count: usize,
    pub memory_count: usize,
    pub plugin_count: usize,
    pub hook_count: usize,
    pub stale_items: Vec<String>,  // memory files older than 30 days
    pub context_ceiling: u64,      // model context window in tokens
    pub pct_used: f64,             // always_tokens / ceiling * 100
    pub duplication: DuplicationReport,
}

#[derive(Debug, Clone)]
pub struct DuplicationReport {
    /// Percentage of always-loaded content that is duplicate (0.0-100.0)
    pub duplicate_pct: f64,
    /// Estimated wasted tokens from duplication
    pub wasted_tokens: u64,
    /// Per-file-pair overlaps, sorted by overlap descending
    pub overlaps: Vec<FileOverlap>,
    /// Total unique lines across all always-loaded files
    pub total_lines: usize,
    /// Lines that appear in 2+ files
    pub duplicate_lines: usize,
}

#[derive(Debug, Clone)]
pub struct FileOverlap {
    pub file_a: String,
    pub file_b: String,
    /// Number of shared non-trivial lines
    pub shared_lines: usize,
    /// Overlap as pct of the smaller file's lines
    pub overlap_pct: f64,
}

pub fn scan() -> BudgetReport {
    let home = dirs::home_dir().unwrap_or_default();
    let claude_dir = home.join(".claude");
    let mut items: Vec<BudgetItem> = Vec::new();

    // 1. CLAUDE.md files in common locations
    scan_claude_md(&home, &mut items);

    // 2. Memory files
    scan_memory(&claude_dir, &mut items);

    // 3. User skills
    scan_skills(&claude_dir.join("skills"), &mut items, Loaded::OnDemand);

    // 4. Installed plugins
    let plugin_count = scan_plugins(&claude_dir.join("plugins"), &mut items);

    // 5. Hooks from settings.json
    let hook_count = scan_hooks(&claude_dir.join("settings.json"));

    // 6. Settings.json itself (permissions, etc.)
    scan_settings(&claude_dir, &mut items);

    // Compute totals
    let always_bytes: u64 = items.iter().filter(|i| i.loaded == Loaded::Always).map(|i| i.bytes).sum();
    let always_tokens: u64 = items.iter().filter(|i| i.loaded == Loaded::Always).map(|i| i.tokens).sum();
    let ondemand_bytes: u64 = items.iter().filter(|i| i.loaded == Loaded::OnDemand).map(|i| i.bytes).sum();
    let ondemand_tokens: u64 = items.iter().filter(|i| i.loaded == Loaded::OnDemand).map(|i| i.tokens).sum();

    let skill_count = items.iter().filter(|i| i.category == Category::Skill).count();
    let memory_count = items.iter().filter(|i| i.category == Category::Memory).count();

    // Stale detection: memory files not modified in 30+ days
    let cutoff = Utc::now() - chrono::Duration::days(30);
    let stale_items: Vec<String> = items.iter()
        .filter(|i| i.category == Category::Memory)
        .filter(|i| i.modified.is_some_and(|m| m < cutoff))
        .map(|i| i.name.clone())
        .collect();

    // Context ceiling: 1M for Opus, 200K default
    let context_ceiling = 1_000_000u64;
    let pct_used = always_tokens as f64 / context_ceiling as f64 * 100.0;

    let duplication = analyze_duplication(&items);

    BudgetReport {
        items,
        always_bytes,
        always_tokens,
        ondemand_bytes,
        ondemand_tokens,
        skill_count,
        memory_count,
        plugin_count,
        hook_count,
        stale_items,
        context_ceiling,
        pct_used,
        duplication,
    }
}

fn scan_claude_md(home: &Path, items: &mut Vec<BudgetItem>) {
    // Check common project roots for CLAUDE.md
    let candidates = [
        home.join("Developer/CLAUDE.md"),
    ];
    for path in &candidates {
        if let Ok(meta) = fs::metadata(path) {
            let bytes = meta.len();
            items.push(BudgetItem {
                name: "CLAUDE.md".into(),
                path: path.display().to_string(),
                bytes,
                tokens: estimate_tokens(bytes),
                category: Category::ClaudeMd,
                loaded: Loaded::Always,
                modified: meta.modified().ok().map(DateTime::<Utc>::from),
            });
        }
    }
}

fn scan_memory(claude_dir: &Path, items: &mut Vec<BudgetItem>) {
    // Scan all project memory directories
    let projects_dir = claude_dir.join("projects");
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let memory_dir = entry.path().join("memory");
            if memory_dir.is_dir() {
                if let Ok(files) = fs::read_dir(&memory_dir) {
                    for file in files.flatten() {
                        let path = file.path();
                        if path.extension().is_some_and(|e| e == "md") {
                            if let Ok(meta) = fs::metadata(&path) {
                                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                                let bytes = meta.len();
                                items.push(BudgetItem {
                                    name,
                                    path: path.display().to_string(),
                                    bytes,
                                    tokens: estimate_tokens(bytes),
                                    category: Category::Memory,
                                    loaded: Loaded::Always,
                                    modified: meta.modified().ok().map(DateTime::<Utc>::from),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

fn scan_skills(skills_dir: &Path, items: &mut Vec<BudgetItem>, loaded: Loaded) {
    if !skills_dir.is_dir() { return; }
    if let Ok(entries) = fs::read_dir(skills_dir) {
        for entry in entries.flatten() {
            let skill_md = entry.path().join("SKILL.md");
            if skill_md.exists() {
                if let Ok(meta) = fs::metadata(&skill_md) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    let bytes = meta.len();
                    items.push(BudgetItem {
                        name,
                        path: skill_md.display().to_string(),
                        bytes,
                        tokens: estimate_tokens(bytes),
                        category: Category::Skill,
                        loaded: loaded.clone(),
                        modified: meta.modified().ok().map(DateTime::<Utc>::from),
                    });
                }
            }
        }
    }
}

fn scan_plugins(plugins_dir: &Path, items: &mut Vec<BudgetItem>) -> usize {
    let mut count = 0;
    if !plugins_dir.is_dir() { return 0; }

    // Count plugin directories
    for subdir in &["marketplaces", "cache"] {
        let dir = plugins_dir.join(subdir);
        if dir.is_dir() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        count += 1;
                    }
                }
            }
        }
    }

    // The skill listing in system-reminder is the main cost
    // Estimate ~80 bytes per skill listing line
    // This is always loaded as part of the system prompt
    let estimated_listing_bytes = count as u64 * 200; // rough: name + description per plugin
    if estimated_listing_bytes > 0 {
        items.push(BudgetItem {
            name: format!("plugin listings ({})", count),
            path: plugins_dir.display().to_string(),
            bytes: estimated_listing_bytes,
            tokens: estimate_tokens(estimated_listing_bytes),
            category: Category::Plugin,
            loaded: Loaded::Always,
            modified: None,
        });
    }

    count
}

fn scan_hooks(settings_path: &Path) -> usize {
    if !settings_path.exists() { return 0; }
    let content = fs::read_to_string(settings_path).unwrap_or_default();
    // Count "hooks" entries (simple parse without full JSON)
    let hook_count = content.matches("\"command\"").count();
    hook_count
}

fn scan_settings(claude_dir: &Path, items: &mut Vec<BudgetItem>) {
    let settings_path = claude_dir.join("settings.json");
    if let Ok(meta) = fs::metadata(&settings_path) {
        let bytes = meta.len();
        items.push(BudgetItem {
            name: "settings.json".into(),
            path: settings_path.display().to_string(),
            bytes,
            tokens: estimate_tokens(bytes),
            category: Category::Settings,
            loaded: Loaded::Always,
            modified: meta.modified().ok().map(DateTime::<Utc>::from),
        });
    }
}

/// Returns true if a line is non-trivial (not blank, not just punctuation/whitespace/markdown fencing)
fn is_meaningful_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() { return false; }
    if trimmed.len() < 4 { return false; }
    // Skip markdown structural lines
    if trimmed.starts_with("---") || trimmed.starts_with("```") || trimmed.starts_with("===") {
        return false;
    }
    // Skip lines that are only punctuation/symbols
    if trimmed.chars().all(|c| !c.is_alphanumeric()) { return false; }
    true
}

/// Normalize a line for comparison: lowercase, collapse whitespace, strip leading markers
fn normalize_line(line: &str) -> String {
    let trimmed = line.trim();
    // Strip common markdown prefixes
    let stripped = trimmed
        .trim_start_matches('#')
        .trim_start_matches('-')
        .trim_start_matches('*')
        .trim_start_matches('>')
        .trim_start_matches('|')
        .trim();
    stripped.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Analyze duplication across always-loaded files using line-level comparison.
fn analyze_duplication(items: &[BudgetItem]) -> DuplicationReport {
    let always_items: Vec<&BudgetItem> = items.iter()
        .filter(|i| i.loaded == Loaded::Always)
        .filter(|i| i.category != Category::Settings && i.category != Category::Plugin)
        .collect();

    // Read file contents and extract meaningful normalized lines
    let mut file_lines: Vec<(String, HashSet<String>, usize)> = Vec::new(); // (name, normalized_lines, total_meaningful)

    for item in &always_items {
        let content = match fs::read_to_string(&item.path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meaningful: HashSet<String> = content.lines()
            .filter(|l| is_meaningful_line(l))
            .map(|l| normalize_line(l))
            .collect();
        let count = meaningful.len();
        if count > 0 {
            file_lines.push((item.name.clone(), meaningful, count));
        }
    }

    // Global line frequency: how many files contain each line
    let mut line_freq: HashMap<String, usize> = HashMap::new();
    for (_, lines, _) in &file_lines {
        for line in lines {
            *line_freq.entry(line.clone()).or_insert(0) += 1;
        }
    }

    let total_lines: usize = file_lines.iter().map(|(_, _, c)| *c).sum();
    let all_unique: HashSet<&String> = line_freq.keys().collect();
    let duplicate_unique_lines: usize = line_freq.values().filter(|&&f| f > 1).count();

    // Count total duplicate line occurrences (each extra copy beyond the first)
    let mut duplicate_line_instances: usize = 0;
    for (_, lines, _) in &file_lines {
        for line in lines {
            if let Some(&freq) = line_freq.get(line) {
                if freq > 1 {
                    // This line exists in multiple files. Count this file's copy
                    // as duplicate (we'll subtract one "original" per line later).
                    duplicate_line_instances += 1;
                }
            }
        }
    }
    // Each duplicated line has (freq) copies but only 1 is "original",
    // so wasted = total_copies - unique_duplicated_lines
    let wasted_lines = duplicate_line_instances.saturating_sub(duplicate_unique_lines);

    // Pairwise overlap
    let mut overlaps: Vec<FileOverlap> = Vec::new();
    for i in 0..file_lines.len() {
        for j in (i+1)..file_lines.len() {
            let shared: usize = file_lines[i].1.intersection(&file_lines[j].1).count();
            if shared > 0 {
                let smaller = file_lines[i].2.min(file_lines[j].2);
                let overlap_pct = if smaller > 0 { shared as f64 / smaller as f64 * 100.0 } else { 0.0 };
                overlaps.push(FileOverlap {
                    file_a: file_lines[i].0.clone(),
                    file_b: file_lines[j].0.clone(),
                    shared_lines: shared,
                    overlap_pct,
                });
            }
        }
    }
    overlaps.sort_by(|a, b| b.shared_lines.cmp(&a.shared_lines));

    let duplicate_pct = if total_lines > 0 {
        wasted_lines as f64 / total_lines as f64 * 100.0
    } else {
        0.0
    };

    let wasted_tokens = estimate_tokens(
        (wasted_lines as f64 * 50.0) as u64  // ~50 bytes avg per meaningful line
    );

    DuplicationReport {
        duplicate_pct,
        wasted_tokens,
        overlaps,
        total_lines: all_unique.len(),
        duplicate_lines: duplicate_unique_lines,
    }
}

/// Format report as CLI text output
pub fn format_report(report: &BudgetReport) -> String {
    let mut out = String::new();

    out.push_str("Context Budget Report\n");
    out.push_str(&"=".repeat(50));
    out.push('\n');

    out.push_str(&format!("\nAlways loaded: {} tokens ({} bytes)\n",
        report.always_tokens, report.always_bytes));
    out.push_str(&format!("On-demand:     {} tokens ({} bytes)\n",
        report.ondemand_tokens, report.ondemand_bytes));
    out.push_str(&format!("Budget used:   {:.1}% of {}K\n",
        report.pct_used, report.context_ceiling / 1000));

    out.push_str(&format!("\nComponents: {} memory, {} skills, {} plugins, {} hooks\n",
        report.memory_count, report.skill_count, report.plugin_count, report.hook_count));

    // Always-loaded breakdown
    out.push_str("\nAlways Loaded:\n");
    let mut always: Vec<&BudgetItem> = report.items.iter()
        .filter(|i| i.loaded == Loaded::Always)
        .collect();
    always.sort_by(|a, b| b.tokens.cmp(&a.tokens));
    for item in &always {
        out.push_str(&format!("  {:>6} tok  {}\n", item.tokens, item.name));
    }

    // On-demand breakdown
    let ondemand: Vec<&BudgetItem> = report.items.iter()
        .filter(|i| i.loaded == Loaded::OnDemand)
        .collect();
    if !ondemand.is_empty() {
        out.push_str("\nOn Demand:\n");
        for item in &ondemand {
            out.push_str(&format!("  {:>6} tok  {}\n", item.tokens, item.name));
        }
    }

    // Duplication analysis
    let dup = &report.duplication;
    out.push_str("\nDuplication:\n");
    out.push_str(&format!("  {:.1}% redundant ({} wasted tok, {} lines in 2+ files)\n",
        dup.duplicate_pct, dup.wasted_tokens, dup.duplicate_lines));

    let top_overlaps: Vec<&FileOverlap> = dup.overlaps.iter()
        .filter(|o| o.shared_lines >= 3)
        .take(8)
        .collect();
    if !top_overlaps.is_empty() {
        out.push_str("  Overlapping pairs:\n");
        for o in &top_overlaps {
            out.push_str(&format!("    {} <> {}  {} shared lines ({:.0}%)\n",
                o.file_a, o.file_b, o.shared_lines, o.overlap_pct));
        }
    }

    // Stale warnings
    if !report.stale_items.is_empty() {
        out.push_str("\nStale (>30 days):\n");
        for name in &report.stale_items {
            out.push_str(&format!("  {}\n", name));
        }
    }

    out
}
