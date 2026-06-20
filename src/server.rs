use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::git::{
    get_commit_diff, get_current_branch, get_git_diff, get_git_log, get_list_of_branches,
    get_list_of_tags, git_diff_compact_summary, git_fetch, GitFlags,
};
use crate::messages::{ClientMsg, ServerMsg, SessionState};

#[derive(Clone)]
#[allow(dead_code)]
pub struct AppState {
    pub repos: Vec<String>,
    pub root: PathBuf,
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<Arc<AppState>>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, app_state: Arc<AppState>) {
    let (mut ws_tx, mut ws_rx) = socket.split();

    let (tx, mut rx) = mpsc::channel::<Vec<ServerMsg>>(10000);
    let (client_tx, mut client_rx) = mpsc::channel::<ClientMsg>(10000);
    let (file_change_tx, mut file_change_rx) = mpsc::channel::<()>(100);

    // Task: forward serialized messages to the WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msgs) = rx.recv().await {
            let text = match serde_json::to_string(&msgs) {
                Ok(t) => t,
                Err(e) => {
                    warn!("Failed to serialize message: {e}");
                    continue;
                }
            };
            debug!("Sending WS: {text}");
            if ws_tx.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    // Task: read from WebSocket and parse into ClientMsg
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Text(text) => {
                    debug!("Received WS: {text}");
                    match serde_json::from_str::<ClientMsg>(&text) {
                        Ok(client_msg) => {
                            if client_tx.send(client_msg).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse client message: {e}, raw: {text}");
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Main handler loop
    let tx2 = tx.clone();
    let handler_task = tokio::spawn(async move {
        let mut session = SessionState::default();
        let mut watcher_handle: Option<tokio::task::JoinHandle<()>> = None;

        // Send initial data
        let _ = tx2
            .send(vec![ServerMsg::Repos {
                repos: app_state.repos.clone(),
            }])
            .await;

        loop {
            tokio::select! {
                Some(msg) = client_rx.recv() => {
                    let state_changed = handle_client_msg(
                        msg,
                        &mut session,
                        &tx2,
                    ).await;

                    if state_changed {
                        let _ = tx2.send(vec![ServerMsg::SessionState { state: session.clone() }]).await;

                        if let Some(ref repo) = session.repo {
                            send_repo_data(repo, session.branch.as_deref(), &session.git_flags, &tx2).await;
                            send_diff_summary(&session, &tx2).await;
                            if !session.open_paths.is_empty() {
                                send_diff(Some(&session.open_paths), &session, &tx2).await;
                            }
                        }

                        // Restart file watcher if watching uncommitted changes
                        if session.commit_a.is_none() {
                            if let Some(ref repo) = session.repo {
                                if let Some(h) = watcher_handle.take() {
                                    h.abort();
                                }
                                let repo_path = repo.clone();
                                let fctx = file_change_tx.clone();
                                watcher_handle = Some(tokio::spawn(async move {
                                    watch_repo(&repo_path, fctx).await;
                                }));
                            }
                        }
                    }
                }
                Some(()) = file_change_rx.recv() => {
                    send_diff_summary(&session, &tx2).await;
                    if !session.open_paths.is_empty() {
                        send_diff(Some(&session.open_paths), &session, &tx2).await;
                    }
                }
                else => break,
            }
        }

        if let Some(h) = watcher_handle.take() {
            h.abort();
        }
    });

    // Wait for any task to finish (connection closed)
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
        _ = handler_task => {},
    }

    info!("WebSocket connection closed");
}

async fn handle_client_msg(
    msg: ClientMsg,
    session: &mut SessionState,
    tx: &mpsc::Sender<Vec<ServerMsg>>,
) -> bool {
    match msg {
        ClientMsg::Heartbeat { .. } | ClientMsg::Ping | ClientMsg::Pong => false,
        ClientMsg::SetCommitA { commit } => {
            session.commit_a = Some(commit);
            true
        }
        ClientMsg::ResetCommitA => {
            session.commit_a = None;
            true
        }
        ClientMsg::SetCommitB { commit } => {
            session.commit_b = Some(commit);
            true
        }
        ClientMsg::ResetCommitB => {
            session.commit_b = None;
            true
        }
        ClientMsg::SwapCommits => {
            std::mem::swap(&mut session.commit_a, &mut session.commit_b);
            true
        }
        ClientMsg::ContextInc => {
            session.git_flags.context_lines += 1;
            true
        }
        ClientMsg::ContextDec => {
            if session.git_flags.context_lines > 0 {
                session.git_flags.context_lines -= 1;
                true
            } else {
                false
            }
        }
        ClientMsg::ContextReset => {
            if session.git_flags.context_lines != 3 {
                session.git_flags.context_lines = 3;
                true
            } else {
                false
            }
        }
        ClientMsg::MaxCountInc => {
            session.git_flags.max_count += 25;
            true
        }
        ClientMsg::MaxCountDec => {
            if session.git_flags.max_count > 25 {
                session.git_flags.max_count -= 25;
                true
            } else {
                false
            }
        }
        ClientMsg::IgnoreAllSpace { value } => {
            session.git_flags.ignore_all_space = value;
            true
        }
        ClientMsg::GetDiff { paths } => {
            send_diff(paths.as_deref(), session, tx).await;
            false
        }
        ClientMsg::GitFetch => {
            if let Some(ref repo) = session.repo {
                let _ = git_fetch(repo).await;
                send_repo_data(repo, session.branch.as_deref(), &session.git_flags, tx).await;
                send_diff_summary(session, tx).await;
                session.open_paths.clear();
            }
            true
        }
        ClientMsg::RepoSelect { repo } => {
            if session.repo.as_deref() != Some(&repo) {
                session.commit_a = None;
                session.commit_b = None;
                session.repo = Some(repo.clone());
                if let Ok(branch) = get_current_branch(&repo).await {
                    session.branch = Some(branch.name);
                }
            }
            true
        }
        ClientMsg::BranchSelect { branch } => {
            if session.branch.as_deref() != Some(&branch) {
                session.branch = Some(branch.clone());
                if let Some(ref repo) = session.repo {
                    if let Ok(commits) =
                        get_git_log(repo, Some(&branch), session.git_flags.max_count).await
                    {
                        let _ = tx.send(vec![ServerMsg::Commits { commits }]).await;
                    }
                }
            }
            true
        }
        ClientMsg::OpenPath { path } => {
            if !session.open_paths.contains(&path) {
                session.open_paths.push(path.clone());
                send_diff(Some(&[path]), session, tx).await;
            }
            true
        }
        ClientMsg::ClosePath { path } => {
            if let Some(pos) = session.open_paths.iter().position(|p| p == &path) {
                session.open_paths.remove(pos);
                true
            } else {
                false
            }
        }
        ClientMsg::SetOpenPaths { paths } => {
            session.open_paths = paths;
            true
        }
        ClientMsg::SetDiffAlgo { algo } => {
            session.git_flags.diff_algo = algo;
            true
        }
    }
}

async fn send_repo_data(
    repo: &str,
    branch: Option<&str>,
    flags: &GitFlags,
    tx: &mpsc::Sender<Vec<ServerMsg>>,
) {
    let mut msgs = Vec::new();

    if let Ok(branches) = get_list_of_branches(repo).await {
        msgs.push(ServerMsg::Branches { branches });
    }
    if let Ok(tags) = get_list_of_tags(repo).await {
        let tags: Vec<_> = tags.into_iter().take(100).collect();
        msgs.push(ServerMsg::Tags { tags });
    }
    if let Ok(commits) = get_git_log(repo, branch, flags.max_count).await {
        msgs.push(ServerMsg::Commits { commits });
    }

    if !msgs.is_empty() {
        let _ = tx.send(msgs).await;
    }
}

async fn send_diff_summary(session: &SessionState, tx: &mpsc::Sender<Vec<ServerMsg>>) {
    let Some(ref repo) = session.repo else {
        return;
    };

    let result = if session.commit_a.is_some() {
        git_diff_compact_summary(
            repo,
            session.commit_a.as_deref(),
            session.commit_b.as_deref(),
        )
        .await
    } else {
        git_diff_compact_summary(repo, None, None).await
    };

    if let Ok(summary) = result {
        let _ = tx.send(vec![ServerMsg::DiffSummary { summary }]).await;
    }
}

async fn send_diff(
    paths: Option<&[String]>,
    session: &SessionState,
    tx: &mpsc::Sender<Vec<ServerMsg>>,
) {
    let Some(ref repo) = session.repo else {
        return;
    };

    let partial = paths.is_some();

    let result = match (&session.commit_a, &session.commit_b) {
        (Some(a), Some(b)) => {
            get_git_diff(repo, Some(a), Some(b), &session.git_flags, paths).await
        }
        (Some(a), None) => get_commit_diff(repo, a, &session.git_flags, paths).await,
        _ => get_git_diff(repo, None, None, &session.git_flags, paths).await,
    };

    if let Ok(diff) = result {
        let _ = tx.send(vec![ServerMsg::Diff { diff, partial }]).await;
    }
}

async fn watch_repo(repo: &str, tx: mpsc::Sender<()>) {
    let (notify_tx, mut notify_rx) = mpsc::channel(100);
    let repo_path = PathBuf::from(repo);

    let mut watcher = match RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(_event) = res {
                let _ = notify_tx.blocking_send(());
            }
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            warn!("Failed to create file watcher: {e}");
            return;
        }
    };

    if let Err(e) = watcher.watch(&repo_path, RecursiveMode::Recursive) {
        warn!("Failed to watch repo {repo}: {e}");
        return;
    }

    info!("Watching repo {repo} for changes");

    loop {
        if notify_rx.recv().await.is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
        while notify_rx.try_recv().is_ok() {}
        let _ = tx.send(()).await;
    }
}
