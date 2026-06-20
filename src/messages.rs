use serde::{Deserialize, Serialize};

use crate::git::{
    Commit, GitBranch, GitDiff, GitDiffAlgorithm, GitDiffSummary, GitFlags, GitTag,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub commit_a: Option<String>,
    pub commit_b: Option<String>,
    pub open_paths: Vec<String>,
    pub git_flags: GitFlags,
}

impl Default for SessionState {
    fn default() -> Self {
        Self {
            repo: None,
            branch: None,
            commit_a: None,
            commit_b: None,
            open_paths: Vec::new(),
            git_flags: GitFlags::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum ServerMsg {
    #[serde(rename = "session-state")]
    SessionState { state: SessionState },
    Repos { repos: Vec<String> },
    Branches { branches: Vec<GitBranch> },
    Tags { tags: Vec<GitTag> },
    Commits { commits: Vec<Commit> },
    Diff { diff: GitDiff, partial: bool },
    #[serde(rename = "diff-summary")]
    DiffSummary { summary: GitDiffSummary },
    Ping,
    Pong,
    Heartbeat { timestamp: u64 },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum ClientMsg {
    Ping,
    Pong,
    Heartbeat { timestamp: u64 },
    #[serde(rename = "set-commit-a")]
    SetCommitA { commit: String },
    #[serde(rename = "set-commit-b")]
    SetCommitB { commit: String },
    #[serde(rename = "reset-commit-a")]
    ResetCommitA,
    #[serde(rename = "reset-commit-b")]
    ResetCommitB,
    #[serde(rename = "swap-commits")]
    SwapCommits,
    #[serde(rename = "context-inc")]
    ContextInc,
    #[serde(rename = "context-dec")]
    ContextDec,
    #[serde(rename = "context-reset")]
    ContextReset,
    #[serde(rename = "max-count-inc")]
    MaxCountInc,
    #[serde(rename = "max-count-dec")]
    MaxCountDec,
    #[serde(rename = "ignore-all-space")]
    IgnoreAllSpace { value: bool },
    #[serde(rename = "repo-select")]
    RepoSelect { repo: String },
    #[serde(rename = "branch-select")]
    BranchSelect { branch: String },
    #[serde(rename = "get-diff")]
    GetDiff { paths: Option<Vec<String>> },
    #[serde(rename = "git-fetch")]
    GitFetch,
    #[serde(rename = "open-path")]
    OpenPath { path: String },
    #[serde(rename = "close-path")]
    ClosePath { path: String },
    #[serde(rename = "set-open-paths")]
    SetOpenPaths { paths: Vec<String> },
    #[serde(rename = "set-diff-algo")]
    SetDiffAlgo { algo: GitDiffAlgorithm },
}
