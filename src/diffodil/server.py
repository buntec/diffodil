import asyncio
import copy
import json
import logging
import os
import uuid
from asyncio import Event, Queue
from contextlib import asynccontextmanager
from dataclasses import asdict, dataclass, field
from datetime import date, datetime

from pydantic_settings import BaseSettings, SettingsConfigDict
from starlette.applications import Starlette
from starlette.routing import Mount, WebSocketRoute
from starlette.staticfiles import StaticFiles
from starlette.websockets import WebSocket

from diffodil.git import (
    Commit,
    GitBranch,
    GitDiff,
    GitDiffSummary,
    GitFlags,
    GitTag,
    find_git_repos,
    get_commit_diff,
    get_current_branch,
    get_git_diff,
    get_git_log,
    get_list_of_branches,
    get_list_of_tags,
    git_diff_compact_summary,
)
from diffodil.utils import setup_logger


@dataclass
class GlobalState:
    repos: list[str] = field(default_factory=list)


@dataclass
class SessionState:
    repo: str | None = None
    branch: str | None = None
    commit_a: str | None = None
    commit_b: str | None = None
    open_paths: list[str] = field(default_factory=list)
    git_flags: GitFlags = field(default_factory=GitFlags)


@dataclass
class MsgBranches:
    branches: list[GitBranch]
    type: str = "branches"


@dataclass
class MsgTags:
    tags: list[GitTag]
    type: str = "tags"


@dataclass
class MsgCommits:
    commits: list[Commit]
    type: str = "commits"


@dataclass
class MsgRepos:
    repos: list[str]
    type: str = "repos"


@dataclass
class MsgRepoSelect:
    repo: str
    type: str = "repo-select"


@dataclass
class MsgBranchSelect:
    branch: str
    type: str = "branch-select"


@dataclass
class MsgPing:
    type: str = "ping"


@dataclass
class MsgPong:
    type: str = "pong"


@dataclass
class MsgHeartbeat:
    timestamp: int
    type: str = "heartbeat"


@dataclass
class MsgGetDiff:
    paths: list[str] | None
    type: str = "get-diff"


@dataclass
class MsgOpenPath:
    path: str
    type: str = "open-path"


@dataclass
class MsgClosePath:
    path: str
    type: str = "close-path"


@dataclass
class MsgSetOpenPaths:
    paths: list[str]
    type: str = "set-open-paths"


@dataclass
class MsgSetCommitA:
    commit: str
    type: str = "set-commit-a"


@dataclass
class MsgResetCommitA:
    type: str = "reset-commit-a"


@dataclass
class MsgSetCommitB:
    commit: str
    type: str = "set-commit-b"


@dataclass
class MsgSwapCommits:
    type: str = "swap-commits"


@dataclass
class MsgResetCommitB:
    type: str = "reset-commit-b"


@dataclass
class MsgDiff:
    diff: GitDiff
    partial: bool
    type: str = "diff"


@dataclass
class MsgDiffSummary:
    summary: GitDiffSummary
    type: str = "diff-summary"


@dataclass
class MsgSessionState:
    state: SessionState
    type: str = "session-state"


@dataclass
class MsgContextInc:
    type: str = "context-inc"


@dataclass
class MsgSetIgnoreAllSpace:
    value: bool
    type: str = "ignore-all-space"


@dataclass
class MsgContextDec:
    type: str = "context-dec"


@dataclass
class MsgContextReset:
    type: str = "context-reset"


type MsgServer = (
    MsgSessionState
    | MsgRepos
    | MsgBranches
    | MsgTags
    | MsgCommits
    | MsgDiff
    | MsgDiffSummary
    | MsgPing
    | MsgPong
    | MsgHeartbeat
)

type MsgClient = (
    MsgGetDiff
    | MsgRepoSelect
    | MsgBranchSelect
    | MsgOpenPath
    | MsgClosePath
    | MsgSetOpenPaths
    | MsgSetCommitA
    | MsgSetCommitB
    | MsgResetCommitA
    | MsgResetCommitB
    | MsgSwapCommits
    | MsgContextInc
    | MsgContextDec
    | MsgContextReset
    | MsgSetIgnoreAllSpace
    | MsgPing
    | MsgPong
    | MsgHeartbeat
)


class JsonEnc(json.JSONEncoder):
    def default(self, o):
        if isinstance(o, (datetime, date)):
            return o.isoformat()
        return super().default(o)


class Settings(BaseSettings):
    root: str = ""
    verbosity: int = 0

    model_config = SettingsConfigDict(env_prefix="DIFFODIL_")


qs_tx: dict[str, Queue[MsgServer]] = {}  # WS send queues
qs_rx: dict[str, Queue[MsgClient]] = {}  # WS receive queues

settings = Settings()

global_state = GlobalState()

logger = logging.getLogger("diffodil")

setup_logger(logger, settings.verbosity)


def decode_client_msg(msg: str) -> MsgClient:
    match json.loads(msg):
        case {"type": "ping"}:
            return MsgPing()
        case {"type": "pong"}:
            return MsgPong()
        case {"type": "heartbeat", "timestamp": timestamp}:
            return MsgHeartbeat(timestamp)
        case {"type": "set-commit-a", "commit": commit}:
            return MsgSetCommitA(commit)
        case {"type": "set-commit-b", "commit": commit}:
            return MsgSetCommitB(commit)
        case {"type": "reset-commit-a"}:
            return MsgResetCommitA()
        case {"type": "reset-commit-b"}:
            return MsgResetCommitB()
        case {"type": "swap-commits"}:
            return MsgSwapCommits()
        case {"type": "context-inc"}:
            return MsgContextInc()
        case {"type": "context-dec"}:
            return MsgContextDec()
        case {"type": "ignore-all-space", "value": value}:
            return MsgSetIgnoreAllSpace(value)
        case {"type": "context-reset"}:
            return MsgContextReset()
        case {"type": "repo-select", "repo": repo}:
            return MsgRepoSelect(repo)
        case {"type": "get-diff", "paths": paths}:
            return MsgGetDiff(paths)
        case {"type": "get-diff"}:
            return MsgGetDiff(None)
        case {"type": "branch-select", "branch": branch}:
            return MsgBranchSelect(branch)
        case {"type": "open-path", "path": path}:
            return MsgOpenPath(path)
        case {"type": "close-path", "path": path}:
            return MsgClosePath(path)
        case {"type": "set-open-paths", "paths": paths}:
            return MsgSetOpenPaths(paths)
        case _:
            raise RuntimeError(f"failed to decode client message: {msg}")


async def ws_broadcast(msg: MsgServer):
    for _, q in qs_tx.items():
        await q.put(msg)


async def websocket_endpoint(websocket: WebSocket):
    await websocket.accept()
    uid = str(uuid.uuid4())

    q_tx: Queue[MsgServer] = asyncio.Queue(10000)
    q_rx: Queue[MsgClient] = asyncio.Queue(10000)

    qs_tx[uid] = q_tx
    qs_rx[uid] = q_rx

    ev_state_change = Event()

    state = SessionState()

    logger.info(f"Opening new WS connection: {uid}")

    async def send_repo_data(repo: str, branch: str | None):
        branches = await get_list_of_branches(repo)
        tags = await get_list_of_tags(repo)
        commits = await get_git_log(repo, branch, 25)
        await q_tx.put(MsgBranches(branches))
        await q_tx.put(MsgTags(tags[:50]))
        await q_tx.put(MsgCommits(commits))
        await q_tx.put(MsgSessionState(state))

    async def send_diff_summary(state: SessionState):
        if state.repo and state.commit_a and state.commit_b:
            diff = await git_diff_compact_summary(
                state.repo,
                state.commit_a,
                state.commit_b,
            )
            await q_tx.put(MsgDiffSummary(diff))
        elif state.repo and state.commit_a:
            diff = await git_diff_compact_summary(state.repo, state.commit_a, None)
            await q_tx.put(MsgDiffSummary(diff))

    async def send_diff(paths: list[str] | None, state: SessionState):
        if state.repo and state.commit_a and state.commit_b:
            diff = await get_git_diff(
                state.repo,
                state.commit_a,
                state.commit_b,
                state.git_flags,
                paths,
            )
            await q_tx.put(MsgDiff(diff, paths is not None))
        elif state.repo and state.commit_a:
            diff = await get_commit_diff(
                state.repo, state.commit_a, state.git_flags, paths
            )
            await q_tx.put(MsgDiff(diff, paths is not None))

    async def handle_client_msg(msg: MsgClient):
        match msg:
            case MsgHeartbeat():
                pass
            case MsgSetCommitA(commit):
                state.commit_a = commit
                ev_state_change.set()
            case MsgResetCommitA():
                state.commit_a = None
                ev_state_change.set()
            case MsgSetCommitB(commit):
                state.commit_b = commit
                ev_state_change.set()
            case MsgResetCommitB():
                state.commit_b = None
                ev_state_change.set()
            case MsgSwapCommits():
                state.commit_a, state.commit_b = state.commit_b, state.commit_a
                ev_state_change.set()
            case MsgContextInc():
                state.git_flags.context_lines += 1
                ev_state_change.set()
            case MsgContextDec():
                if state.git_flags.context_lines > 0:
                    state.git_flags.context_lines -= 1
                    ev_state_change.set()
            case MsgContextReset():
                if state.git_flags.context_lines != 3:
                    state.git_flags.context_lines = 3
                    ev_state_change.set()
            case MsgSetIgnoreAllSpace(value):
                state.git_flags.ignore_all_space = value
                ev_state_change.set()
            case MsgGetDiff(paths):
                await send_diff(paths, state)
            case MsgRepoSelect(repo):
                if state.repo != repo:
                    state.commit_a = None
                    state.commit_b = None
                    state.repo = repo
                    branch = await get_current_branch(repo)
                    state.branch = branch.name
                    await send_repo_data(repo, None)
                    ev_state_change.set()
            case MsgBranchSelect(branch):
                if state.branch != branch:
                    state.branch = branch
                    if state.repo:
                        commits = await get_git_log(state.repo, branch, 25)
                        await q_tx.put(MsgCommits(commits))
                    ev_state_change.set()
            case MsgOpenPath(path):
                if path not in state.open_paths:
                    state.open_paths.append(path)
                ev_state_change.set()
            case MsgClosePath(path):
                if path in state.open_paths:
                    state.open_paths.remove(path)
                    ev_state_change.set()
            case MsgSetOpenPaths(paths):
                state.open_paths = paths
                ev_state_change.set()
            case _:
                logger.warning(f"unhandled WS message: {msg}")

    async def send_init_data():
        await q_tx.put(MsgRepos(global_state.repos))

    async def handle_state_changes_loop():
        while True:
            state_prev = copy.deepcopy(state)
            await ev_state_change.wait()

            await q_tx.put(MsgSessionState(state))

            if (
                state.repo != state_prev.repo
                or state.branch != state_prev.branch
                or state.commit_a != state_prev.commit_a
                or state.commit_b != state_prev.commit_a
            ):
                await send_diff_summary(state)
                if state.open_paths:
                    await send_diff(state.open_paths, state)

            else:
                new_paths = set(state.open_paths) - set(state_prev.open_paths)
                if new_paths:
                    await send_diff(list(new_paths), state)

            ev_state_change.clear()
            await asyncio.sleep(0.1)  # debounce

    # send in chunks with a maximum delay (in seconds)
    async def send_loop(max_chunk_size: int, max_delay: float):
        buffer = []
        timeout = False
        while True:
            try:
                msg = await asyncio.wait_for(q_tx.get(), max_delay)
                buffer.append(asdict(msg))
                q_tx.task_done()
            except TimeoutError:
                timeout = True
            if len(buffer) >= max_chunk_size or (timeout and buffer):
                text = json.dumps(buffer, cls=JsonEnc)
                logger.info(f"sending WS message: {text}")
                await websocket.send_text(text)
                buffer.clear()
                timeout = False

    async def recv_loop():
        while True:
            msg = await websocket.receive_text()
            logger.info(f"received WS message: {msg}")
            await q_rx.put(decode_client_msg(msg))

    async def handle_client_msg_loop():
        while True:
            msg = await q_rx.get()
            await handle_client_msg(msg)

    try:
        async with asyncio.TaskGroup() as tg:
            tg.create_task(send_init_data())
            tg.create_task(recv_loop())
            tg.create_task(handle_client_msg_loop())
            tg.create_task(handle_state_changes_loop())
            tg.create_task(send_loop(max_chunk_size=100, max_delay=0.1))
    except* Exception as e:
        logger.info(f"WS connection {uid} exception in task group: {e.exceptions}")
        await websocket.close(1011)
    finally:
        del qs_tx[uid]
        del qs_rx[uid]
        logger.info(f"Closing WS connection {uid}")


@asynccontextmanager
async def lifespan(_):
    settings.root = os.path.abspath(settings.root)
    global_state.repos = find_git_repos(settings.root)
    yield


routes = [
    WebSocketRoute("/ws", websocket_endpoint),
    Mount("/", app=StaticFiles(html=True, packages=["diffodil"]), name="static"),
]


app = Starlette(routes=routes, lifespan=lifespan)
