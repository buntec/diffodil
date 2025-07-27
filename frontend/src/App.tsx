import { useState, useRef, useEffect, useReducer } from 'react'
import { Theme, SegmentedControl, ScrollArea, Callout, Switch, Select, IconButton, Badge, Box, Code, Blockquote, Grid, Flex, Text, Button } from "@radix-ui/themes";
import { ExclamationTriangleIcon, ChevronDownIcon, WidthIcon, ResetIcon, Cross1Icon, PlusCircledIcon, MinusCircledIcon } from "@radix-ui/react-icons"
import { Accordion, Toast } from "radix-ui";
import './App.css'

function prettyDate(date: any) {
  return new Intl.DateTimeFormat("en-US", {
    dateStyle: "full",
    timeStyle: "short",
  }).format(date);
}

export function useWebSocket(url: string, onMessage: any, onError: any, onClose: any) {
  const ws = useRef<WebSocket>(null);
  const heartbeatInterval = useRef<number>(null);

  useEffect(() => {
    function connect() {
      ws.current = new WebSocket(url);

      ws.current.onopen = () => {
        console.log("WebSocket connected");

        // Start heartbeat
        heartbeatInterval.current = setInterval(() => {
          if (ws.current && ws.current.readyState === WebSocket.OPEN) {
            ws.current.send(
              JSON.stringify({ type: "heartbeat", timestamp: Date.now() }),
            );
          }
        }, 10000);
      };

      ws.current.onmessage = (event) => {
        try {
          // console.info(`Received WS message: ${event.data}`)
          const data = JSON.parse(event.data);
          onMessage(data);
        } catch (error) {
          console.warn(`Failed to parse WS message to JSON: ${event.data}`)
        }
      };

      ws.current.onerror = (err) => {
        console.error("WebSocket error", err);
        onError(err);
      };

      ws.current.onclose = (event) => {
        console.log("WebSocket closed: ", event);
        console.log("Attempting to reconnect WebSocket...");
        onClose(event);
        setTimeout(() => connect(), 3000);
      };
    }

    // if we connect immediately, the first attempt fails (for the dev server)
    setTimeout(() => connect(), 500);

    return () => {
      if (ws.current) {
        ws.current.close();
      }
      if (heartbeatInterval.current) {
        clearInterval(heartbeatInterval.current);
      }
    };
  }, [url]);

  const sendMsg = (o: any) => {
    if (ws.current?.readyState === WebSocket.OPEN) {
      ws.current.send(JSON.stringify(o));
    }
  };

  return { sendMsg };
}

const reducer = (state: State, action: any): State => {
  if (!action.type) { return { ...state }; }

  // console.log(`dispatch: ${action.type}`);

  switch (action.type) {
    case "repos":
      return { ...state, repos: action.repos };
    case "branches":
      return { ...state, branches: action.branches };
    case "tags":
      return { ...state, tags: action.tags };
    case "commits":
      return { ...state, commits: action.commits };
    case "session-state":
      if (state.session?.branch !== action.state.branch || state.session?.commit_a != action.state.commit_a || state.session?.commit_b != action.state.commit_b) {
        return { ...state, session: action.state, diffPartial: undefined };
      }
      return { ...state, session: action.state }
    case "diff":
      if (action.partial) {
        if (state.diffPartial) {
          const diff: GitPartialDiff = { ...state.diffPartial }
          const files = new Map(diff.files)
          action.diff.files.forEach((file: GitDiffFile) => {
            files.set(file.file_path, file)
          })
          diff.files = files
          return { ...state, diffPartial: diff };
        } else {
          const kvs = action.diff?.files?.map((file: GitDiffFile) => [file.file_path, file])
          const files: Map<string, GitDiffFile> = new Map([...kvs])
          return { ...state, diffPartial: { from_commit: action.diff.from_commit, to_commit: action.diff.to_commit, files: files } };
        }
      } else {
        return { ...state, diff: action.diff };
      }
    case "diff-summary":
      return { ...state, diffSummary: action.summary };
  }
  return { ...state };
}

type GitFlags = {
  context_lines: number
  diff_algo: string
  ignore_all_space: boolean
}

type SessionState = {
  repo: string;
  branch: string;
  commit_a: string;
  commit_b: string;
  open_paths: string[];
  git_flags: GitFlags;
}

type GitCommit = {
  short_hash: string
  summary: string
  body: string
  author: string
  date: string
}

interface GitBranch {
  name: string
  is_current: boolean
  is_remote: boolean
  remote: string | null
  points_to: string | null
}

interface GitTag {
  name: string
  message: string | null
}

type GitDiffHunk = {
  header: string
  old_start: number
  old_count: number
  new_start: number
  new_count: number
  content: string[]
  added_lines: number
  removed_lines: number
}

interface GitDiffFile {
  file_path: string
  change_type: string
  hunks: GitDiffHunk[]
}

interface GitDiff {
  from_commit: string
  to_commit: string
  files: GitDiffFile[]
}

interface GitPartialDiff {
  from_commit: string
  to_commit: string
  files: Map<string, GitDiffFile>
}

interface GitFileChange {
  path: string
  change_type: string
  old_path?: string
  is_binary: boolean
  additions?: number
  deletions?: number
  changes?: number
}

interface GitDiffSummary {
  commit_a: string
  commit_b: string
  files: GitFileChange[]
  total_files_changed: number
  total_additions: number
  total_deletions: number
}

interface State {
  repos: string[]
  branches: GitBranch[]
  tags: GitTag[]
  commits: GitCommit[]
  session?: SessionState
  diff?: GitDiff
  diffSummary?: GitDiffSummary
  diffPartial?: GitPartialDiff
}

type WhitespaceSwitchProps = {
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}

function WhitespaceSwitch({ checked, onCheckedChange }: WhitespaceSwitchProps) {
  return (
    <Box>
      <Text as="label" size="2">
        <Flex gap="2">
          <Switch size="1" checked={checked} onCheckedChange={onCheckedChange} /> Ignore all space
        </Flex>
      </Text>
    </Box>
  )
}

type AppearanceSwitchProps = {
  appearance: AppearanceType
  setAppearance: (t: AppearanceType) => void
}

function AppearanceSwitch({ appearance, setAppearance }: AppearanceSwitchProps) {
  return (
    <Box>
      <Text as="label" size="2">
        <Flex gap="2">
          <Switch size="1" checked={appearance === 'dark'} onCheckedChange={(checked) => setAppearance(checked ? 'dark' : 'light')} /> Dark mode
        </Flex>
      </Text>
    </Box>
  )
}

type RepoSelectProps = {
  repos: string[]
  repo?: string
  onRepoChange: (repo: string) => void
}

function RepoSelect({ repos, repo, onRepoChange }: RepoSelectProps) {
  return (
    <Select.Root size="2" onValueChange={onRepoChange} value={repo ? repo : ''}>
      <Select.Trigger variant="soft" placeholder="Select repo" />
      <Select.Content>
        {repos.map((repo) =>
          <Select.Item key={repo} value={repo}>{repo}</Select.Item>
        )
        }
      </Select.Content>
    </Select.Root>
  )
}


type BranchSelectProps = {
  branches: GitBranch[]
  branch?: string
  onBranchChange: (branch: string) => void
}

function BranchSelect({ branches, branch, onBranchChange }: BranchSelectProps) {
  return (
    <Select.Root size="2" onValueChange={onBranchChange} value={branch ? branch : ''}>
      <Select.Trigger variant="soft" placeholder="Select branch" />
      <Select.Content>
        {branches.map((branch) =>
          <Select.Item key={branch.name} value={branch.name}>{branch.name}</Select.Item>
        )
        }
      </Select.Content>
    </Select.Root>
  )
}

type CommitsSelectProps = {
  state: State;
  sendMsg: (msg: any) => void
}

function CommitsSelect({ state, sendMsg }: CommitsSelectProps) {
  return (<Flex direction="row" gap="1" align="center">
    {
      state.session?.commit_a &&
      <Flex direction="row" gap="2" p="1" style={{ background: "var(--accent-2)" }}>
        <Badge size="2" color="cyan">A</Badge>
        <Text color="cyan">{state.session?.commit_a}</Text>
        <IconButton variant="soft" size="1" onClick={() => sendMsg({ type: "reset-commit-a" })} ><Cross1Icon /></IconButton>
      </Flex>
    }
    {(state.session?.commit_a || state.session?.commit_b) && <IconButton size="1" variant="soft" onClick={() => sendMsg({ type: "swap-commits" })}><WidthIcon /></IconButton>}
    {
      state.session?.commit_b &&
      <Flex direction="row" gap="2" p="1" style={{ background: "var(--accent-2)" }}>
        <Badge size="2" color="orange">B</Badge>
        <Text color="orange">{state.session?.commit_b}</Text>
        <IconButton variant="soft" size="1" onClick={() => sendMsg({ type: "reset-commit-b" })} ><Cross1Icon /></IconButton>
      </Flex>
    }
  </Flex>
  )
}


type ContextControlProps = {
  value: number
  onInc: () => void;
  onDec: () => void;
  onReset: () => void;
}

function ContextControl({ value, onInc, onDec, onReset }: ContextControlProps) {
  return (
    <Flex gap="1" direction="row">
      <Text wrap="nowrap">Context: {value}</Text>
      <IconButton variant="soft" size="1" onClick={onInc}><PlusCircledIcon /></IconButton >
      <IconButton variant="soft" size="1" onClick={onDec}><MinusCircledIcon /></IconButton>
      <IconButton variant="soft" size="1" onClick={onReset}><ResetIcon /></IconButton>
    </Flex>
  )
}

type ButtonABProps = {
  isSelectA: boolean
  isSelectB: boolean
  selectA: () => void;
  selectB: () => void;
  unselectA: () => void;
  unselectB: () => void;

}

function ButtonAB({ isSelectA, isSelectB, selectA, unselectA, selectB, unselectB }: ButtonABProps) {
  return (
    <Flex direction="row" gap="1">
      <Button size="1" variant="soft" color={isSelectA ? "cyan" : "gray"} onClick={isSelectA ? unselectA : selectA}>A</Button>
      <Button size="1" variant="soft" color={isSelectB ? "orange" : "gray"} onClick={isSelectB ? unselectB : selectB}>B</Button>
    </Flex>
  )

}

type TagProps = {
  tag: GitTag;
  session?: SessionState;
  sendMsg: (msg: any) => void;
}

function Tag({ tag, session, sendMsg }: TagProps) {
  return (<Flex direction="row" align="center" gap="1" mb="1" mx="2" >
    <ButtonAB
      isSelectA={session?.commit_a == tag.name}
      isSelectB={session?.commit_b == tag.name}
      selectA={() => sendMsg({ type: 'set-commit-a', commit: tag.name })}
      unselectA={() => sendMsg({ type: 'reset-commit-a', commit: tag.name })}
      selectB={() => sendMsg({ type: 'set-commit-b', commit: tag.name })}
      unselectB={() => sendMsg({ type: 'reset-commit-b', commit: tag.name })}
    />
    <Text size="2" color={tag.name === session?.commit_a ?
      'cyan' : tag.name === session?.commit_b ?
        'orange' : undefined} >{tag.name}</Text>
  </Flex>
  )
}

type CommitProps = {
  commit: GitCommit;
  session?: SessionState;
  sendMsg: (msg: any) => void;
}

function Commit({ commit, session, sendMsg }: CommitProps) {
  return (
    <Flex direction="column" m="2" key={commit.short_hash}>
      <Flex direction="row" gap="2">
        <ButtonAB
          isSelectA={session?.commit_a == commit.short_hash}
          isSelectB={session?.commit_b == commit.short_hash}
          selectA={() => sendMsg({ type: 'set-commit-a', commit: commit.short_hash })}
          unselectA={() => sendMsg({ type: 'reset-commit-a', commit: commit.short_hash })}
          selectB={() => sendMsg({ type: 'set-commit-b', commit: commit.short_hash })}
          unselectB={() => sendMsg({ type: 'reset-commit-b', commit: commit.short_hash })}
        />
        <Text color={commit.short_hash === session?.commit_a ?
          'cyan' : commit.short_hash === session?.commit_b ?
            'orange' : undefined}>{commit.short_hash}</Text>
      </Flex>
      <Text size="1">{prettyDate(Date.parse(commit.date))}</Text>
      <Text size="1">{commit.author}</Text>
      <Box display="contents" className="commit-hover-wrapper">
        <Box>
          <Blockquote className="commit-summary" size="1" color="crimson">{commit.summary}{commit.body ? '(...)' : null}</Blockquote>
        </Box>
        <Box>
          <Blockquote className="commit-body" size="1" color="gold">{commit.body}</Blockquote>
        </Box>
      </Box>
    </Flex>
  )
}

type FileDiffProps = {
  file?: GitDiffFile
}


function FileDiff({ file }: FileDiffProps) {
  return (file && <Box>
    {file.hunks.map((hunk, i) =>
      <Box key={i}>
        <Code size="1">{hunk.header}</Code>
        <Flex direction="column">
          {hunk.content.map((line, i) => <Code className="code-diff" size="1" wrap="wrap"
            color={line.startsWith('-') ? 'red' : line.startsWith('+') ? 'green' : 'gray'} key={i}>
            {line}
          </Code>)}
        </Flex>
      </Box>
    )}
  </Box>)
}


type DiffProps = {
  session: SessionState
  diff?: GitPartialDiff
  summary: GitDiffSummary
  sendMsg: (msg: any) => void
}

function colorFromChangeType(change_type: string): "gold" | "red" | "green" | "blue" {
  switch (change_type) {
    case "modified":
      return "gold"
    case "deleted":
      return "red"
    case "added":
      return "green"
  }
  return "blue";
}

function Diff({ session, summary, diff, sendMsg }: DiffProps) {
  return (
    <ScrollArea type="auto" scrollbars="vertical">
      <Box gridArea="diff" m="2">
        <Accordion.Root type="multiple"
          value={session.open_paths}
          onValueChange={(paths: string[]) => sendMsg({ type: 'set-open-paths', paths: paths })}
          className="AccordionRoot" >
          {summary.files.map((file, i) =>
            <Accordion.Item key={i} value={file.path} className="AccordionItem">
              <Accordion.Header className="AccordionHeader" asChild>
                <div>
                  <Accordion.Trigger className="AccordionTrigger" asChild >
                    <Flex direction="row" gap="1" px="2">
                      <Button variant="ghost" color={colorFromChangeType(file.change_type)} size="2">{file.path} ({file.change_type}) </Button>
                      <ChevronDownIcon className="AccordionChevron" aria-hidden />
                    </Flex>
                  </ Accordion.Trigger >
                </div>
              </Accordion.Header>
              <Accordion.Content >
                {diff && <FileDiff file={diff.files.get(file.path)} />}
              </Accordion.Content >
            </Accordion.Item>
          )}
        </Accordion.Root>
      </Box>
    </ScrollArea>
  )
}

type WSErrorToastProps = {
  open: boolean;
  setOpen: (open: boolean) => void;
}

function WSErrorToast({ open, setOpen }: WSErrorToastProps) {
  return (
    <Toast.Root className="ToastRoot" open={open} onOpenChange={setOpen}>
      <Toast.Description asChild>
        <Callout.Root variant="soft" color="red">
          <Callout.Icon>
            <ExclamationTriangleIcon />
          </Callout.Icon>
          <Callout.Text>
            WS connection failed.
          </Callout.Text>
        </Callout.Root>
      </Toast.Description>
    </Toast.Root>
  )
}

type CommitSelectType = "commits" | "tags"

type AppearanceType = "light" | "dark"

const wsUrl = `${window.location.protocol === "https:" ? "wss" : "ws"}://${window.location.host}/ws`;

const initialState: State = { repos: [], branches: [], tags: [], commits: [] }

function App() {
  const [state, dispatch] = useReducer<State, any>(reducer, initialState);

  const [wsError, setWsError] = useState<boolean>(false);

  const [commitSelectType, setCommitSelectType] = useState<CommitSelectType>("commits");

  const [appearance, setAppearance] = useState<AppearanceType>("light")

  const { sendMsg } = useWebSocket(wsUrl,
    (msg: any) => {
      if (Array.isArray(msg)) {
        msg.forEach(dispatch);
      } else {
        dispatch(msg);
      }
    }, (_: any) => setWsError(true), (_: any) => setWsError(true)
  );

  const CommitTypeSelect = <SegmentedControl.Root m="2" size="1" value={commitSelectType} onValueChange={(value: CommitSelectType) => setCommitSelectType(value)}>
    <SegmentedControl.Item value="commits">Commits</SegmentedControl.Item>
    <SegmentedControl.Item value="tags">Tags</SegmentedControl.Item>
  </SegmentedControl.Root>


  const Commits = <Flex gridArea="commits" direction="column" justify="start" overflow="auto">
    {state.session?.branch && CommitTypeSelect}
    <ScrollArea type="auto" scrollbars="vertical">
      <Flex direction="column">
        {
          ((t: CommitSelectType) => {
            switch (t) {
              case "tags":
                return state.tags.map((tag) => <Tag key={tag.name} tag={tag} session={state.session} sendMsg={sendMsg} />)
              case "commits":
                return state.commits.map((commit) => <Commit key={commit.short_hash} commit={commit} session={state.session} sendMsg={sendMsg} />)
            }
          })(commitSelectType)
        }
      </Flex>
    </ScrollArea>
  </Flex>





  const Ribbon = <Flex gridArea="ribbon" gap="4" justify="between" width="100%" align="center" p="2" wrap="wrap">
    <RepoSelect
      repo={state.session?.repo}
      repos={state.repos} onRepoChange={(repo) => sendMsg({ type: "repo-select", repo: repo })}
    />
    {
      state.session?.branch &&
      <BranchSelect
        branch={state.session?.branch}
        branches={state.branches}
        onBranchChange={(branch) => sendMsg({ type: "branch-select", branch: branch })}
      />
    }
    <CommitsSelect state={state} sendMsg={sendMsg} />
    {
      state.session &&
      <ContextControl
        value={state.session?.git_flags.context_lines}
        onReset={() => sendMsg({ type: "context-reset" })}
        onInc={() => sendMsg({ type: "context-inc" })}
        onDec={() => sendMsg({ type: "context-dec" })} />
    }
    {
      state.session?.git_flags &&
      <WhitespaceSwitch
        checked={state.session?.git_flags.ignore_all_space}
        onCheckedChange={(checked: boolean) => sendMsg({ type: 'ignore-all-space', value: checked })}
      />
    }
    <AppearanceSwitch appearance={appearance} setAppearance={setAppearance} />
  </Flex>


  return (
    <Theme appearance={appearance}>
      <Toast.Provider>
        <Grid height="100vh" overflow="hidden" columns="minmax(auto, 1fr) 3fr" rows="min-content" areas={` "ribbon ribbon" "commits diff" `}>
          {Ribbon}
          {Commits}
          {state.diffSummary && state.session?.commit_a &&
            <Diff session={state.session} sendMsg={sendMsg} diff={state.diffPartial} summary={state.diffSummary} />}
        </Grid>
        <WSErrorToast open={wsError} setOpen={setWsError} />
        <Toast.Viewport />
      </Toast.Provider>
    </Theme>
  )
}

export default App
