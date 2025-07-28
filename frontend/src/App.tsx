import { useState, useReducer } from 'react'
import { Theme, SegmentedControl, ScrollArea, Callout, Switch, Select, IconButton, Badge, Box, Code, Blockquote, Grid, Flex, Text, Button } from "@radix-ui/themes";
import { UpdateIcon, MoonIcon, ExclamationTriangleIcon, ChevronDownIcon, WidthIcon, ResetIcon, Cross1Icon, PlusCircledIcon, MinusCircledIcon } from "@radix-ui/react-icons"
import { Accordion, Toast } from "radix-ui";
import { useWebSocket } from './WebSocket.tsx'
import type { GitDiffAlgo, GitFlags, GitCommit, GitBranch, GitPartialDiff, GitTag, GitDiffFile, GitDiff, GitDiffSummary } from './Git.tsx'
import { prettyDate } from './Utils.tsx'
import './App.css'


type SessionState = {
  repo: string;
  branch: string;
  commit_a: string;
  commit_b: string;
  open_paths: string[];
  git_flags: GitFlags;
}

type State = {
  repos: string[]
  branches: GitBranch[]
  tags: GitTag[]
  commits: GitCommit[]
  session?: SessionState
  diff?: GitDiff
  diffSummary?: GitDiffSummary
  diffPartial?: GitPartialDiff
}

const reducer = (state: State, action: any): State => {
  if (!action.type) {
    console.warn(`action is missing type: ${action}`)
    return { ...state };
  }

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
        }
        const kvs = action.diff?.files?.map((file: GitDiffFile) => [file.file_path, file])
        const files: Map<string, GitDiffFile> = new Map([...kvs])
        return { ...state, diffPartial: { from_commit: action.diff.from_commit, to_commit: action.diff.to_commit, files: files } };
      }
      return { ...state, diff: action.diff };
    case "diff-summary":
      return { ...state, diffSummary: action.summary };
  }

  console.warn(`unknown action type: ${action.type}`)
  return { ...state };
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
          <Switch color="pink" size="1" checked={checked} onCheckedChange={onCheckedChange} /> <Code color={checked ? "pink" : "gray"} size="1">--ignore-all-space</Code>
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
        <Flex gap="2" align="center">
          <Switch color="gold" size="1" checked={appearance === 'dark'} onCheckedChange={(checked) => setAppearance(checked ? 'dark' : 'light')} /> <MoonIcon />
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


type AlgoSelectProps = {
  algo?: GitDiffAlgo
  onAlgoChange: (algo: GitDiffAlgo) => void
}

function AlgoSelect({ algo, onAlgoChange }: AlgoSelectProps) {
  return (
    <Flex direction="row" align="center" gap="1">
      <Code color={algo == "myers" ? "gray" : "pink"} size="1">--diff-algorithm</Code>
      <Select.Root size="1" onValueChange={onAlgoChange} value={algo ? algo : ''}>
        <Select.Trigger color="gray" variant="soft" />
        <Select.Content >
          {['myers', 'patience', 'histogram', 'minimal'].map((name) =>
            <Select.Item key={name} value={name}>{name}</Select.Item>
          )}
        </Select.Content>
      </Select.Root>
    </Flex>
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
      <Select.Trigger color="gold" variant="soft" placeholder="Select branch" />
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
    <Flex gap="1" direction="row" align="center">
      <IconButton color="gray" variant="soft" size="1" onClick={onInc}><PlusCircledIcon /></IconButton >
      <IconButton color="gray" variant="soft" size="1" onClick={onDec}><MinusCircledIcon /></IconButton>
      <IconButton color="gray" variant="soft" size="1" onClick={onReset}><ResetIcon /></IconButton>
      <Code color={value === 3 ? "gray" : "pink"} size="1" wrap="nowrap">--unified={value}</Code>
    </Flex>
  )
}


type MaxCountControlProps = {
  value: number
  onInc: () => void;
  onDec: () => void;
}

function MaxCountControl({ value, onInc, onDec }: MaxCountControlProps) {
  return (
    <Flex gap="1" direction="row" align="center">
      <IconButton color="gray" variant="soft" size="1" onClick={onInc}><PlusCircledIcon /></IconButton >
      <IconButton color="gray" variant="soft" size="1" onClick={onDec}><MinusCircledIcon /></IconButton>
      <Code color={value === 25 ? "gray" : "pink"} size="1" wrap="nowrap">--max-count={value}</Code>
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
    {file.hunks.map((hunk, i) => {
      let k_new = 0;
      let k_old = 0;
      return <Box key={i}>
        <Code size="1">{hunk.header}</Code>
        <Flex direction="column">
          {hunk.content.map((line, j) => {
            const isDel = line.startsWith('-')
            const isAdd = line.startsWith('+')

            const diffLine = <Flex align="center" className="diff-line" key={j}>
              <Code className="diff-line-number" color="gray" size="1">{isAdd ? '' : hunk.old_start + k_old}</Code>
              <Code className="diff-line-number" color="gray" size="1">{isDel ? '' : hunk.new_start + k_new}</Code>
              <Code className="code-diff" size="1" wrap="wrap" color={isDel ? 'red' : isAdd ? 'green' : 'gray'}>{line}</Code>
            </Flex>

            if (isDel) { k_old += 1 }
            else if (isAdd) { k_new += 1 }
            else { k_old += 1; k_new += 1 }

            return diffLine;
          })}
        </Flex>
      </Box>
    }
    )}
  </Box>)
}


type DiffProps = {
  session: SessionState
  diff?: GitPartialDiff
  summary: GitDiffSummary
  sendMsg: (msg: any) => void
}

function colorFromChangeType(change_type: string) {
  switch (change_type) {
    case "modified":
      return "gold"
    case "deleted":
      return "red"
    case "added":
      return "green"
    case "renamed":
      return "indigo"
  }
  return "blue";
}

function Diff({ session, summary, diff, sendMsg }: DiffProps) {
  return (
    <ScrollArea type="auto" scrollbars="both">
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
                      <Button
                        variant="ghost"
                        color={colorFromChangeType(file.change_type)}
                        size="2">
                        <Text>{file.old_path ? `${file.old_path} => ${file.path}` : file.path}</Text>
                        <Text>({file.change_type})</Text>
                        {file.changes && <Text>{file.changes}</Text>}
                        {file.additions != undefined && file.additions > 0 && <Code size="1" color="green">{'+'.repeat(file.additions)}</Code>}
                        {file.deletions != undefined && file.deletions > 0 && <Code size="1" color="red" >{'-'.repeat(file.deletions)}</Code>}
                      </Button>
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

function App() {
  const [state, dispatch] = useReducer<State, any>(reducer, { repos: [], branches: [], tags: [], commits: [] });
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

  const CommitTypeSelect = <SegmentedControl.Root
    m="2" size="1"
    value={commitSelectType}
    onValueChange={(value: CommitSelectType) => setCommitSelectType(value)}
  >
    <SegmentedControl.Item value="commits">Commits</SegmentedControl.Item>
    <SegmentedControl.Item value="tags">Tags</SegmentedControl.Item>
  </SegmentedControl.Root>

  const Commits = <Flex gridArea="commits" direction="column" justify="start" overflow="auto">
    {state.session?.branch && CommitTypeSelect}
    <ScrollArea type="auto" scrollbars="vertical">
      <Flex direction="column" px="1">
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

  const GitFetchButton = <Button variant="soft" size="1" onClick={() => sendMsg({ type: 'git-fetch' })}>Fetch<UpdateIcon /></Button>

  const Ribbon = <Flex gridArea="ribbon" gap="2" justify="between" align="center" p="2" wrap="wrap">
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
    {
      state.session && GitFetchButton
    }
    <CommitsSelect state={state} sendMsg={sendMsg} />
    {
      state.session &&
      <AlgoSelect
        algo={state.session?.git_flags.diff_algo}
        onAlgoChange={(algo) => sendMsg({ type: 'set-diff-algo', algo: algo })}
      />
    }
    {
      state.session && <MaxCountControl
        value={state.session?.git_flags.max_count}
        onInc={() => sendMsg({ type: "max-count-inc" })}
        onDec={() => sendMsg({ type: "max-count-dec" })} />
    }
    {
      state.session && <ContextControl
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
        <Grid height="100vh"
          overflow="hidden" columns="minmax(auto, 1fr) 3fr"
          rows="min-content" areas={`"ribbon ribbon" "commits diff"`}>
          {Ribbon}
          {Commits}
          {state.session && state.diffSummary &&
            <Diff session={state.session} sendMsg={sendMsg} diff={state.diffPartial} summary={state.diffSummary} />}
        </Grid>
        <WSErrorToast open={wsError} setOpen={setWsError} />
        <Toast.Viewport />
      </Toast.Provider>
    </Theme>
  )
}

export default App
