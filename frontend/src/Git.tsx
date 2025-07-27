export type GitDiffAlgo = "myers" | "minimal" | "patience" | "histogram"

export type GitFileChangeType = "added" | "deleted" | "modified" | "renamed" | "copied"

export type GitFlags = {
  context_lines: number
  diff_algo: GitDiffAlgo
  ignore_all_space: boolean
}

export type GitCommit = {
  short_hash: string
  summary: string
  body: string
  author: string
  date: string
}

export type GitBranch = {
  name: string
  is_current: boolean
  is_remote: boolean
  remote: string | null
  points_to: string | null
}

export type GitTag = {
  name: string
  message: string | null
}

export type GitDiffHunk = {
  header: string
  old_start: number
  old_count: number
  new_start: number
  new_count: number
  content: string[]
  added_lines: number
  removed_lines: number
}

export type GitDiffFile = {
  file_path: string
  change_type: GitFileChangeType
  hunks: GitDiffHunk[]
}

export type GitDiff = {
  from_commit: string
  to_commit: string
  files: GitDiffFile[]
}

export type GitPartialDiff = {
  from_commit: string
  to_commit: string
  files: Map<string, GitDiffFile>
}

export type GitFileChange = {
  path: string
  change_type: GitFileChangeType
  old_path?: string
  is_binary: boolean
  additions?: number
  deletions?: number
  changes?: number
}

export type GitDiffSummary = {
  commit_a: string
  commit_b: string
  files: GitFileChange[]
  total_files_changed: number
  total_additions: number
  total_deletions: number
}
