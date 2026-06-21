use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GitFileChangeType {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum GitDiffAlgorithm {
    Myers,
    Minimal,
    Patience,
    Histogram,
}

impl Default for GitDiffAlgorithm {
    fn default() -> Self {
        Self::Myers
    }
}

impl std::fmt::Display for GitDiffAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Myers => write!(f, "myers"),
            Self::Minimal => write!(f, "minimal"),
            Self::Patience => write!(f, "patience"),
            Self::Histogram => write!(f, "histogram"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitFlags {
    pub max_count: u32,
    pub context_lines: u32,
    pub diff_algo: GitDiffAlgorithm,
    pub ignore_all_space: bool,
}

impl Default for GitFlags {
    fn default() -> Self {
        Self {
            max_count: 25,
            context_lines: 3,
            diff_algo: GitDiffAlgorithm::default(),
            ignore_all_space: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitBranch {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub remote: Option<String>,
    pub points_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub short_hash: String,
    pub summary: String,
    pub body: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitTag {
    pub name: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub new_count: u32,
    pub content: Vec<String>,
    pub added_lines: u32,
    pub removed_lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffFile {
    pub file_path: String,
    pub change_type: String,
    pub hunks: Vec<DiffHunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiff {
    pub from_commit: Option<String>,
    pub to_commit: Option<String>,
    pub files: Vec<DiffFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: GitFileChangeType,
    pub old_path: Option<String>,
    pub is_binary: bool,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    pub changes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitDiffSummary {
    pub commit_a: Option<String>,
    pub commit_b: Option<String>,
    pub files: Vec<FileChange>,
    pub total_files_changed: u32,
    pub total_additions: u32,
    pub total_deletions: u32,
}

pub fn find_git_repos(root: &Path) -> Vec<RepoEntry> {
    let mut paths = Vec::new();
    find_git_repos_recursive(root, &mut paths);
    group_repos_by_worktree(&paths)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub path: String,
    pub branch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoEntry {
    pub path: String,
    pub worktrees: Vec<Worktree>,
}

fn get_worktrees(repo_path: &str) -> Vec<Worktree> {
    let output = std::process::Command::new("git")
        .arg("worktree")
        .arg("list")
        .arg("--porcelain")
        .current_dir(repo_path)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_branch: Option<String> = None;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(prev_path) = current_path.take() {
                worktrees.push(Worktree {
                    path: prev_path,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(path.to_string());
            current_branch = None;
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(branch.to_string());
        }
    }
    if let Some(path) = current_path {
        worktrees.push(Worktree {
            path,
            branch: current_branch,
        });
    }

    worktrees
}

fn group_repos_by_worktree(paths: &[String]) -> Vec<RepoEntry> {
    use std::collections::HashSet;

    let path_set: HashSet<&str> = paths.iter().map(|s| s.as_str()).collect();
    let mut claimed: HashSet<&str> = HashSet::new();
    let mut entries: Vec<RepoEntry> = Vec::new();

    for path in paths {
        if claimed.contains(path.as_str()) {
            continue;
        }

        let worktrees = get_worktrees(path);

        // The first entry from `git worktree list` is always the main worktree
        let main_path = worktrees
            .first()
            .map(|wt| wt.path.clone())
            .unwrap_or_else(|| path.clone());

        let relevant: Vec<Worktree> = worktrees
            .into_iter()
            .filter(|wt| path_set.contains(wt.path.as_str()))
            .collect();

        if relevant.len() <= 1 {
            entries.push(RepoEntry {
                path: path.clone(),
                worktrees: Vec::new(),
            });
        } else {
            for wt in &relevant {
                if let Some(p) = paths.iter().find(|p| **p == wt.path) {
                    claimed.insert(p.as_str());
                }
            }

            entries.push(RepoEntry {
                path: main_path,
                worktrees: relevant,
            });
        }
    }

    entries
}

fn find_git_repos_recursive(dir: &Path, repos: &mut Vec<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut has_git = false;
    let mut subdirs: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name();
        if name == ".git" {
            has_git = true;
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        }
    }

    if has_git {
        repos.push(dir.to_string_lossy().to_string());
    } else {
        for subdir in subdirs {
            find_git_repos_recursive(&subdir, repos);
        }
    }
}

pub fn parse_git_branches(output: &str) -> Vec<GitBranch> {
    let mut branches = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let is_current = line.starts_with("* ");
        let line = line.strip_prefix("* ").unwrap_or(line);
        let line = line.strip_prefix("+ ").unwrap_or(line);

        if line.contains("->") {
            let parts: Vec<&str> = line.splitn(2, "->").collect();
            let name_part = parts[0].trim();
            let target = parts[1].trim();
            let is_remote = name_part.starts_with("remotes/");
            let remote = if is_remote {
                name_part.split('/').nth(1).map(|s| s.to_string())
            } else {
                None
            };
            let name = name_part
                .strip_prefix("remotes/")
                .unwrap_or(name_part)
                .to_string();
            branches.push(GitBranch {
                name,
                is_current,
                is_remote,
                remote,
                points_to: Some(target.to_string()),
            });
        } else {
            let is_remote = line.starts_with("remotes/");
            let remote = if is_remote {
                line.split('/').nth(1).map(|s| s.to_string())
            } else {
                None
            };
            let name = line.strip_prefix("remotes/").unwrap_or(line).to_string();
            branches.push(GitBranch {
                name,
                is_current,
                is_remote,
                remote,
                points_to: None,
            });
        }
    }

    branches
}

pub fn parse_git_tags(output: &str) -> Vec<GitTag> {
    let mut tags = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let name = parts.next().unwrap_or("").to_string();
        let message = parts.next().map(|s| s.to_string());

        tags.push(GitTag { name, message });
    }

    tags
}

pub async fn git_fetch(repo: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(["fetch", "--all"])
        .current_dir(repo)
        .output()
        .await
        .map_err(|e| format!("Failed to run git fetch: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git fetch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

pub async fn get_list_of_branches(repo: &str) -> Result<Vec<GitBranch>, String> {
    let output = Command::new("git")
        .args(["branch", "--list", "--all", "--no-color"])
        .current_dir(repo)
        .output()
        .await
        .map_err(|e| format!("Failed to run git branch: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git branch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_git_branches(&stdout))
}

pub async fn get_current_branch(repo: &str) -> Result<GitBranch, String> {
    let branches = get_list_of_branches(repo).await?;
    branches
        .into_iter()
        .find(|b| b.is_current)
        .ok_or_else(|| format!("Failed to get current branch for repo {repo}"))
}

pub async fn get_list_of_tags(repo: &str) -> Result<Vec<GitTag>, String> {
    let output = Command::new("git")
        .args(["tag", "--list", "--no-color"])
        .current_dir(repo)
        .output()
        .await
        .map_err(|e| format!("Failed to run git tag: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git tag failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_git_tags(&stdout))
}

pub async fn get_git_log(
    repo: &str,
    branch: Option<&str>,
    max_count: u32,
) -> Result<Vec<Commit>, String> {
    const SEP1: &str = "<<<><<>>>";
    const SEP2: &str = "><><><<>>";

    let format_str = format!("{SEP1}%h{SEP2}%an{SEP2}%ad{SEP2}%s{SEP2}%b");

    let mut cmd = Command::new("git");
    cmd.args([
        "log",
        &format!("--max-count={max_count}"),
        &format!("--pretty=format:{format_str}"),
        "--date=iso",
    ])
    .current_dir(repo);

    if let Some(branch) = branch {
        cmd.arg(branch);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run git log: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git log failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut commits = Vec::new();

    for record in stdout.split(SEP1) {
        if record.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = record.splitn(5, SEP2).collect();
        if parts.len() < 5 {
            continue;
        }
        commits.push(Commit {
            short_hash: parts[0].trim().to_string(),
            author: parts[1].trim().to_string(),
            date: parts[2].trim().to_string(),
            summary: parts[3].trim().to_string(),
            body: parts[4].trim().to_string(),
        });
    }

    Ok(commits)
}

fn parse_hunk_header(header: &str) -> Option<(u32, u32, u32, u32)> {
    let re_pattern = header.strip_prefix("@@ -")?;
    let at_pos = re_pattern.find(" @@")?;
    let ranges = &re_pattern[..at_pos];

    let parts: Vec<&str> = ranges.splitn(2, " +").collect();
    if parts.len() != 2 {
        return None;
    }

    let old_parts: Vec<&str> = parts[0].splitn(2, ',').collect();
    let new_parts: Vec<&str> = parts[1].splitn(2, ',').collect();

    let old_start = old_parts[0].parse::<u32>().ok()?;
    let old_count = old_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let new_start = new_parts[0].parse::<u32>().ok()?;
    let new_count = new_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);

    Some((old_start, old_count, new_start, new_count))
}

fn parse_diff_output(stdout: &str) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut current_file: Option<DiffFile> = None;
    let mut current_hunk: Option<DiffHunk> = None;

    for line in stdout.lines() {
        if line.starts_with("diff --git") {
            if let Some(hunk) = current_hunk.take() {
                if let Some(ref mut file) = current_file {
                    file.hunks.push(hunk);
                }
            }
            if let Some(file) = current_file.take() {
                files.push(file);
            }
            current_file = Some(DiffFile {
                file_path: String::new(),
                change_type: "modified".to_string(),
                hunks: Vec::new(),
            });
        } else if line.starts_with("new file mode") {
            if let Some(ref mut file) = current_file {
                file.change_type = "added".to_string();
            }
        } else if line.starts_with("deleted file mode") {
            if let Some(ref mut file) = current_file {
                file.change_type = "deleted".to_string();
            }
        } else if (line.starts_with("--- ") || line.starts_with("+++ "))
            && current_file
                .as_ref()
                .is_some_and(|f| f.file_path.is_empty())
        {
            if let Some(ref mut file) = current_file {
                if let Some(path) = line.strip_prefix("+++ b/") {
                    file.file_path = path.to_string();
                } else if let Some(path) = line.strip_prefix("--- a/") {
                    file.file_path = path.to_string();
                }
            }
        } else if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                if let Some(ref mut file) = current_file {
                    file.hunks.push(hunk);
                }
            }
            if let Some((old_start, old_count, new_start, new_count)) =
                parse_hunk_header(line.trim())
            {
                current_hunk = Some(DiffHunk {
                    header: line.trim().to_string(),
                    old_start,
                    old_count,
                    new_start,
                    new_count,
                    content: Vec::new(),
                    added_lines: 0,
                    removed_lines: 0,
                });
            }
        } else if let Some(ref mut hunk) = current_hunk {
            hunk.content.push(line.to_string());
            if line.starts_with('+') && !line.starts_with("+++") {
                hunk.added_lines += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                hunk.removed_lines += 1;
            }
        }
    }

    if let Some(hunk) = current_hunk.take() {
        if let Some(ref mut file) = current_file {
            file.hunks.push(hunk);
        }
    }
    if let Some(file) = current_file.take() {
        files.push(file);
    }

    files
}

pub async fn get_git_diff(
    repo: &str,
    commit_a: Option<&str>,
    commit_b: Option<&str>,
    flags: &GitFlags,
    paths: Option<&[String]>,
    cached: bool,
) -> Result<GitDiff, String> {
    let mut cmd = Command::new("git");
    cmd.args([
        "diff",
        "--patch",
        "--no-color",
        "--find-renames",
        "--find-copies",
        &format!("--unified={}", flags.context_lines),
        &format!("--diff-algorithm={}", flags.diff_algo),
    ])
    .current_dir(repo);

    if cached {
        cmd.arg("--cached");
    }

    if flags.ignore_all_space {
        cmd.arg("--ignore-all-space");
    }

    let (effective_a, effective_b) = match (commit_a, commit_b) {
        (Some(a), Some(b)) => (Some(a.to_string()), Some(b.to_string())),
        (Some(a), None) => (Some(format!("{a}^")), Some(a.to_string())),
        _ => (None, None),
    };

    if let Some(ref a) = effective_a {
        cmd.arg(a);
    }
    if let Some(ref b) = effective_b {
        cmd.arg(b);
    }

    if let Some(paths) = paths {
        cmd.arg("--");
        for p in paths {
            cmd.arg(p);
        }
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run git diff: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = parse_diff_output(&stdout);

    Ok(GitDiff {
        from_commit: effective_a,
        to_commit: effective_b,
        files,
    })
}

pub async fn get_commit_diff(
    repo: &str,
    commit: &str,
    flags: &GitFlags,
    paths: Option<&[String]>,
) -> Result<GitDiff, String> {
    let mut cmd = Command::new("git");
    cmd.args([
        "show",
        "--patch",
        "--no-color",
        "--find-renames",
        "--find-copies",
        &format!("--unified={}", flags.context_lines),
        &format!("--diff-algorithm={}", flags.diff_algo),
        "--pretty=format:",
    ])
    .current_dir(repo);

    if flags.ignore_all_space {
        cmd.arg("--ignore-all-space");
    }

    cmd.arg(commit);

    if let Some(paths) = paths {
        cmd.arg("--");
        for p in paths {
            cmd.arg(p);
        }
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run git show: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git show failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let files = parse_diff_output(&stdout);

    Ok(GitDiff {
        from_commit: Some(commit.to_string()),
        to_commit: Some(commit.to_string()),
        files,
    })
}

pub fn parse_compact_summary_line(line: &str) -> Option<FileChange> {
    let line = line.trim();
    if line.is_empty() || !line.contains('|') {
        return None;
    }

    let (file_part, changes_part) = line.split_once('|')?;
    let file_part = file_part.trim();
    let changes_part = changes_part.trim();

    let mut file_change = FileChange {
        path: String::new(),
        change_type: GitFileChangeType::Modified,
        old_path: None,
        is_binary: false,
        additions: None,
        deletions: None,
        changes: None,
    };

    if file_part.contains(" => ") {
        let (lhs, rhs) = file_part.split_once(" => ").unwrap();
        if lhs.contains('{') && rhs.contains('}') {
            let (prefix, old_path) = lhs.split_once('{').unwrap();
            let (new_path, suffix) = rhs.split_once('}').unwrap();
            file_change.old_path = Some(format!("{}{}{}", prefix, old_path.trim(), suffix));
            file_change.path = format!("{}{}{}", prefix, new_path.trim(), suffix);
        } else {
            file_change.old_path = Some(lhs.trim().to_string());
            file_change.path = rhs.trim().to_string();
        }
        file_change.change_type = GitFileChangeType::Renamed;
    } else if let Some(name) = file_part.strip_suffix(" (new)") {
        file_change.path = name.trim().to_string();
        file_change.change_type = GitFileChangeType::Added;
    } else if let Some(name) = file_part.strip_suffix(" (gone)") {
        file_change.path = name.trim().to_string();
        file_change.change_type = GitFileChangeType::Deleted;
    } else {
        file_change.path = file_part.to_string();
        file_change.change_type = GitFileChangeType::Modified;
    }

    if changes_part.starts_with("Bin") {
        file_change.is_binary = true;
    } else {
        let parts: Vec<&str> = changes_part.split_whitespace().collect();
        if let Some(first) = parts.first() {
            if let Ok(total) = first.parse::<u32>() {
                let plus_count = changes_part.chars().filter(|&c| c == '+').count() as u32;
                let minus_count = changes_part.chars().filter(|&c| c == '-').count() as u32;
                file_change.additions = Some(plus_count);
                file_change.deletions = Some(minus_count);
                file_change.changes = Some(total);
            }
        }
    }

    Some(file_change)
}

pub async fn git_diff_compact_summary(
    repo: &str,
    commit_a: Option<&str>,
    commit_b: Option<&str>,
    cached: bool,
) -> Result<GitDiffSummary, String> {
    let mut cmd = Command::new("git");
    cmd.args(["diff", "--compact-summary", "--stat=10000000"])
        .current_dir(repo);

    if cached {
        cmd.arg("--cached");
    }

    let (effective_a, effective_b) = match (commit_a, commit_b) {
        (Some(a), Some(b)) => (Some(a.to_string()), Some(b.to_string())),
        (Some(a), None) => (Some(format!("{a}^")), Some(a.to_string())),
        _ => (None, None),
    };

    if let Some(ref a) = effective_a {
        cmd.arg(a);
    }
    if let Some(ref b) = effective_b {
        cmd.arg(b);
    }

    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Failed to run git diff: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "Git diff --compact-summary failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut files = Vec::new();
    let mut total_additions = 0u32;
    let mut total_deletions = 0u32;

    for line in stdout.lines() {
        if let Some(file_change) = parse_compact_summary_line(line) {
            if let Some(a) = file_change.additions {
                total_additions += a;
            }
            if let Some(d) = file_change.deletions {
                total_deletions += d;
            }
            files.push(file_change);
        }
    }

    let total_files_changed = files.len() as u32;

    Ok(GitDiffSummary {
        commit_a: effective_a,
        commit_b: effective_b,
        files,
        total_files_changed,
        total_additions,
        total_deletions,
    })
}

pub struct UntrackedFileContent {
    pub content: Option<String>,
    pub is_binary: bool,
}

pub fn read_untracked_file(repo: &str, path: &str) -> UntrackedFileContent {
    let full_path = Path::new(repo).join(path);
    match std::fs::read(&full_path) {
        Ok(bytes) => {
            if bytes.iter().take(8000).any(|&b| b == 0) {
                UntrackedFileContent {
                    content: None,
                    is_binary: true,
                }
            } else {
                match String::from_utf8(bytes) {
                    Ok(s) => UntrackedFileContent {
                        content: Some(s),
                        is_binary: false,
                    },
                    Err(_) => UntrackedFileContent {
                        content: None,
                        is_binary: true,
                    },
                }
            }
        }
        Err(_) => UntrackedFileContent {
            content: None,
            is_binary: false,
        },
    }
}

pub async fn get_untracked_files(repo: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(repo)
        .output()
        .await
        .map_err(|e| format!("Failed to run git ls-files: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}
