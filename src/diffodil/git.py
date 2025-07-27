import asyncio
import logging
import os
import re
from dataclasses import dataclass, field
from datetime import datetime
from enum import StrEnum
from typing import List, Optional, Tuple

logger = logging.getLogger("diffodil")


class GitFileChangeType(StrEnum):
    ADDED = "added"
    DELETED = "deleted"
    MODIFIED = "modified"
    RENAMED = "renamed"
    COPIED = "copied"


class GitDiffAlgorithm(StrEnum):
    MYERS = "myers"  # currently this is the default
    MINIMAL = "minimal"
    PATIENCE = "patience"
    HISTOGRAM = "histogram"


@dataclass
class GitFlags:
    max_count: int = 25
    context_lines: int = 3
    diff_algo: GitDiffAlgorithm = GitDiffAlgorithm.MYERS
    ignore_all_space: bool = False


@dataclass
class GitBranch:
    name: str
    is_current: bool
    is_remote: bool
    remote: Optional[str] = None
    points_to: Optional[str] = None  # For symbolic refs like HEAD -> origin/main


@dataclass
class Commit:
    short_hash: str
    summary: str
    body: str
    author: str
    date: datetime


@dataclass
class GitTag:
    name: str
    message: Optional[str] = None


@dataclass
class DiffHunk:
    header: str
    old_start: int
    old_count: int
    new_start: int
    new_count: int
    content: List[str] = field(default_factory=list)
    added_lines: int = 0
    removed_lines: int = 0


@dataclass
class DiffFile:
    file_path: str
    change_type: str
    hunks: List[DiffHunk] = field(default_factory=list)


@dataclass
class GitDiff:
    from_commit: str
    to_commit: str
    files: List[DiffFile] = field(default_factory=list)


@dataclass
class FileChange:
    path: str
    change_type: GitFileChangeType
    old_path: Optional[str] = None  # For renames/copies
    mode_change: Optional[Tuple[str, str]] = None  # (old_mode, new_mode)
    is_binary: bool = False
    additions: Optional[int] = None
    deletions: Optional[int] = None
    changes: Optional[int] = None


@dataclass
class GitDiffSummary:
    commit_a: str
    commit_b: str
    files: List[FileChange]
    total_files_changed: int
    total_additions: int
    total_deletions: int


def find_git_repos(root):
    git_repos = []

    for dirpath, dirnames, _ in os.walk(root):
        if ".git" in dirnames:
            git_repos.append(dirpath)
            dirnames.clear()  # don't recurse into repos

    return git_repos


def parse_git_branches(output: str) -> List[GitBranch]:
    branches = []

    for line in output.strip().splitlines():
        line = line.strip()

        # Detect current branch (starts with '* ')
        is_current = line.startswith("*")
        if is_current:
            line = line[2:].strip()

        # Handle symbolic refs (e.g., remotes/origin/HEAD -> origin/main)
        if "->" in line:
            name_part, target = map(str.strip, line.split("->", 1))
            is_remote = name_part.startswith("remotes/")
            remote = name_part.split("/")[1] if is_remote else None
            name = name_part.removeprefix("remotes/")
            branches.append(
                GitBranch(
                    name=name,
                    is_current=is_current,
                    is_remote=is_remote,
                    remote=remote,
                    points_to=target,
                )
            )
        else:
            is_remote = line.startswith("remotes/")
            remote = line.split("/")[1] if is_remote else None
            name = line.removeprefix("remotes/")
            branches.append(
                GitBranch(
                    name=name, is_current=is_current, is_remote=is_remote, remote=remote
                )
            )

    return branches


def parse_git_tags(output: str) -> List[GitTag]:
    tags = []

    for line in output.strip().splitlines():
        # Split on first occurrence of whitespace (in case message is included)
        if not line.strip():
            continue

        parts = line.strip().split(None, 1)
        name = parts[0]
        message = parts[1] if len(parts) > 1 else None

        tags.append(GitTag(name=name, message=message))

    return tags


async def get_list_of_branches(repo: str) -> list[GitBranch]:
    """Run `git branch --list --all` and parse its output to get the list of all branches"""

    cmd = ["git", "branch", "--list", "--all", "--no-color"]

    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, cwd=repo
    )

    stdout_b, stderr_b = await proc.communicate()

    stdout = stdout_b.decode()
    stderr = stderr_b.decode()

    if proc.returncode != 0:
        raise RuntimeError(f"Git branch failed: {stderr}")

    logger.debug(f"git stdout: {stdout}")

    return parse_git_branches(stdout)


async def get_current_branch(repo: str) -> GitBranch:
    branches = await get_list_of_branches(repo)
    for branch in branches:
        if branch.is_current:
            return branch
    raise RuntimeError(f"failed to get current branch for repo {repo}")


async def get_list_of_tags(repo: str) -> list[GitTag]:
    """Run `git tag --list` and parse its output to get the list of all tags"""

    cmd = ["git", "tag", "--list", "--no-color"]

    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, cwd=repo
    )

    stdout_b, stderr_b = await proc.communicate()

    stdout = stdout_b.decode()
    stderr = stderr_b.decode()

    if proc.returncode != 0:
        raise RuntimeError(f"Git log failed: {stderr}")

    logger.debug(f"git stdout: {stdout}")

    return parse_git_tags(stdout)


async def get_git_log(
    repo: str, branch: str | None, max_count: int = 10
) -> List[Commit]:
    """Run `git log` with appropriate arguments and parse output into a list of `Commit`s."""

    SEP1 = "<<<><<>>>"
    SEP2 = "><><><<>>"
    format_str = f"{SEP1}%h{SEP2}%an{SEP2}%ad{SEP2}%s{SEP2}%b"

    cmd = [
        "git",
        "log",
        f"--max-count={max_count}",
        f"--pretty=format:{format_str}",
        "--date=iso",
    ]

    if branch:
        cmd.append(branch)

    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, cwd=repo
    )

    stdout_b, stderr_b = await proc.communicate()

    stderr = stderr_b.decode()
    stdout = stdout_b.decode()

    if proc.returncode != 0:
        raise RuntimeError(f"Git log failed: {stderr}")

    logger.debug(f"git log stderr: {stderr}")
    logger.debug(f"git log stdout: {stdout}")

    commits: List[Commit] = []
    for record in stdout.strip().split(SEP1):
        if not record:
            continue
        short_hash, author, date, summary, body = record.split(SEP2)
        commits.append(
            Commit(
                short_hash=short_hash.strip(),
                author=author.strip(),
                date=datetime.fromisoformat(date.strip()),
                summary=summary.strip(),
                body=body.strip(),
            )
        )

    return commits


def parse_hunk_header(header: str) -> tuple:
    """Extract line ranges from hunk headers like: @@ -10,7 +12,9 @@"""
    match = re.match(r"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))?", header)
    if match:
        old_start = int(match.group(1))
        old_count = int(match.group(2) or 1)
        new_start = int(match.group(3))
        new_count = int(match.group(4) or 1)
        return old_start, old_count, new_start, new_count
    raise ValueError(f"Invalid hunk header: {header}")


async def get_git_diff(
    repo: str,
    commit_a: str,
    commit_b: str,
    flags: GitFlags,
    paths: list[str] | None = None,
) -> GitDiff:
    """Run `git diff` on two commits and return the parsed output."""

    cmd = [
        "git",
        "diff",
        "--patch",
        "--no-color",
        "--find-renames",
        "--find-copies",
        f"--unified={flags.context_lines}",
        f"--diff-algorithm={flags.diff_algo}",
    ]

    if flags.ignore_all_space:
        cmd.append("--ignore-all-space")

    cmd.extend(
        [
            commit_a,
            commit_b,
        ]
    )

    if paths:
        cmd.append("--")
        cmd.extend(paths)

    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, cwd=repo
    )

    stdout_b, stderr_b = await proc.communicate()

    stderr = stderr_b.decode()
    stdout = stdout_b.decode()

    if proc.returncode != 0:
        raise RuntimeError(f"Git diff failed: {stderr}")

    logger.debug(f"git log stderr: {stderr}")
    logger.debug(f"git log stdout: {stdout}")

    files: List[DiffFile] = []
    current_file: Optional[DiffFile] = None
    current_hunk: Optional[DiffHunk] = None

    for line in stdout.splitlines():
        if line.startswith("diff --git"):
            if current_hunk and current_file:
                current_file.hunks.append(current_hunk)
            if current_file:
                files.append(current_file)
            current_file = DiffFile(file_path="", change_type="modified")
            current_hunk = None

        elif line.startswith("new file mode"):
            if current_file:
                current_file.change_type = "added"
        elif line.startswith("deleted file mode"):
            if current_file:
                current_file.change_type = "deleted"
        elif line.startswith("--- ") or line.startswith("+++ "):
            if current_file and not current_file.file_path:
                if line.startswith("+++ b/"):
                    current_file.file_path = line[6:]
                elif line.startswith("--- a/"):
                    current_file.file_path = line[6:]

        elif line.startswith("@@"):
            if current_hunk and current_file:
                current_file.hunks.append(current_hunk)
            old_start, old_count, new_start, new_count = parse_hunk_header(line.strip())
            current_hunk = DiffHunk(
                header=line.strip(),
                old_start=old_start,
                old_count=old_count,
                new_start=new_start,
                new_count=new_count,
            )

        else:
            if current_hunk:
                current_hunk.content.append(line)
                if line.startswith("+") and not line.startswith("+++"):
                    current_hunk.added_lines += 1
                elif line.startswith("-") and not line.startswith("---"):
                    current_hunk.removed_lines += 1

    if current_hunk and current_file:
        current_file.hunks.append(current_hunk)
    if current_file:
        files.append(current_file)

    return GitDiff(from_commit=commit_a, to_commit=commit_b, files=files)


async def get_commit_diff(
    repo: str, commit: str, flags: GitFlags, paths: list[str] | None = None
) -> GitDiff:
    """Run `git show` on the given commit and return the parsed output."""

    cmd = [
        "git",
        "show",
        "--patch",
        "--no-color",
        "--find-renames",
        "--find-copies",
        f"--unified={flags.context_lines}",
        f"--diff-algorithm={flags.diff_algo}",
        "--pretty=format:",  # exclude commit metadata
    ]

    if flags.ignore_all_space:
        cmd.append("--ignore-all-space")

    if paths:
        cmd.append("--")
        cmd.extend(paths)

    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, cwd=repo
    )

    stdout_b, stderr_b = await proc.communicate()

    stderr = stderr_b.decode()
    stdout = stdout_b.decode()

    if proc.returncode != 0:
        raise RuntimeError(f"Git show failed: {stderr}")

    logger.debug(f"git show stderr: {stderr}")
    logger.debug(f"git show stdout: {stdout}")

    files: List[DiffFile] = []
    current_file: Optional[DiffFile] = None
    current_hunk: Optional[DiffHunk] = None

    for line in stdout.splitlines():
        if line.startswith("diff --git"):
            if current_hunk and current_file:
                current_file.hunks.append(current_hunk)
            if current_file:
                files.append(current_file)
            current_file = DiffFile(file_path="", change_type="modified")
            current_hunk = None

        elif line.startswith("new file mode"):
            if current_file:
                current_file.change_type = "added"
        elif line.startswith("deleted file mode"):
            if current_file:
                current_file.change_type = "deleted"
        elif line.startswith("--- ") or line.startswith("+++ "):
            if current_file and not current_file.file_path:
                if line.startswith("+++ b/"):
                    current_file.file_path = line[6:]
                elif line.startswith("--- a/"):
                    current_file.file_path = line[6:]

        elif line.startswith("@@"):
            if current_hunk and current_file:
                current_file.hunks.append(current_hunk)
            old_start, old_count, new_start, new_count = parse_hunk_header(line.strip())
            current_hunk = DiffHunk(
                header=line.strip(),
                old_start=old_start,
                old_count=old_count,
                new_start=new_start,
                new_count=new_count,
            )

        else:
            if current_hunk:
                current_hunk.content.append(line)
                if line.startswith("+") and not line.startswith("+++"):
                    current_hunk.added_lines += 1
                elif line.startswith("-") and not line.startswith("---"):
                    current_hunk.removed_lines += 1

    if current_hunk and current_file:
        current_file.hunks.append(current_hunk)
    if current_file:
        files.append(current_file)

    return GitDiff(from_commit=commit, to_commit=commit, files=files)


def parse_compact_summary_line(line: str) -> Optional[FileChange]:
    """Parse a single line from `git diff --compact-summary` output."""

    line = line.strip()
    if not line:
        return None

    # Pattern for parsing the compact summary format
    # Examples:
    # " file.txt | 10 +++++-----"
    # " new_file.py (new) | 25 +++++++++++++++++++++++++"
    # " old_file.txt (gone) | 5 -----"
    # " renamed.txt => new_name.txt | 0"
    # " file.bin | Bin 0 -> 1024 bytes"

    # Split on the pipe character
    if "|" not in line:
        return None

    file_part, changes_part = line.split("|", 1)
    file_part = file_part.strip()
    changes_part = changes_part.strip()

    # Initialize file change object
    file_change = FileChange(path="", change_type=GitFileChangeType.MODIFIED)

    # Parse file path and detect change type
    if " => " in file_part:
        # Rename or copy
        old_path, new_path = file_part.split(" => ", 1)
        file_change.old_path = old_path.strip()
        file_change.path = new_path.strip()
        file_change.change_type = GitFileChangeType.RENAMED
    elif file_part.endswith(" (new)"):
        # New file
        file_change.path = file_part[:-6].strip()
        file_change.change_type = GitFileChangeType.ADDED
    elif file_part.endswith(" (gone)"):
        # Deleted file
        file_change.path = file_part[:-7].strip()
        file_change.change_type = GitFileChangeType.DELETED
    else:
        # Modified file
        file_change.path = file_part
        file_change.change_type = GitFileChangeType.MODIFIED

    # Parse changes part
    if changes_part.startswith("Bin"):
        # Binary file
        file_change.is_binary = True
        # Extract byte changes if present
        bin_match = re.search(r"Bin (\d+) -> (\d+) bytes", changes_part)
        if bin_match:
            # For binary files, we could store byte changes, but compact-summary doesn't show +/-
            pass
    else:
        # Text file with +/- changes
        # Extract number and count + and - symbols
        parts = changes_part.split()
        if parts:
            try:
                total_changes = int(parts[0])
                plus_count = changes_part.count("+")
                minus_count = changes_part.count("-")

                file_change.additions = plus_count
                file_change.deletions = minus_count
                file_change.changes = total_changes
            except ValueError:
                pass

    return file_change


async def git_diff_compact_summary(
    repo: str, commit_a: str, commit_b: str | None
) -> GitDiffSummary:
    """Run git diff --compact-summary for a given commit and return the parsed output."""

    if not commit_b:
        commit_b = commit_a
        commit_a = f"{commit_b}^"

    cmd = ["git", "diff", "--compact-summary", commit_a, commit_b]

    proc = await asyncio.create_subprocess_exec(
        *cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, cwd=repo
    )

    stdout_b, stderr_b = await proc.communicate()

    stderr = stderr_b.decode()
    stdout = stdout_b.decode()

    if proc.returncode != 0:
        raise RuntimeError(f"Git diff --compact-summary failed: {stderr}")

    lines = stdout.split("\n")

    files = []
    total_additions = 0
    total_deletions = 0

    for line in lines:
        file_change = parse_compact_summary_line(line)
        if file_change:
            files.append(file_change)
            if file_change.additions:
                total_additions += file_change.additions
            if file_change.deletions:
                total_deletions += file_change.deletions

    return GitDiffSummary(
        commit_a=commit_a,
        commit_b=commit_b,
        files=files,
        total_files_changed=len(files),
        total_additions=total_additions,
        total_deletions=total_deletions,
    )
