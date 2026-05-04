export type SessionStatus = "running" | "waiting" | "idle";

export interface SessionState {
  id: string;           // session uuid, filename without .jsonl
  project: string;      // last segment of cwd from first JSONL line
  cwd: string;          // full cwd path
  status: SessionStatus;
  last_output: string;  // last assistant message content, max 500 chars; "Running: {tool_name}" for tool_use
  tool_count: number;   // count of tool_use events in file
  recent_tools: string[]; // last 5 tool names used in session
  tty: string;           // tty device path (e.g. /dev/ttys001), empty if unknown
  started_at: string;   // timestamp of first event
  modified_at: string;  // file last modified timestamp
  provider: string;     // agent provider id: "claude", "codex", etc.
  input_tokens: number;          // cumulative input tokens parsed from JSONL
  output_tokens: number;         // cumulative output tokens parsed from JSONL
  cache_read_tokens: number;     // cumulative cache-read tokens
  cache_creation_tokens: number; // cumulative cache-creation tokens
  model: string;                 // model id (e.g. "claude-sonnet-4-6"); empty for opencode
}

// Last-fire token snapshot for a pipe rule. Updated on every pipe-fired event.
export interface PipeTokenState {
  payload_tokens: number;  // tokens injected into the target session
  source_tokens: number;   // input + output tokens of the source session at fire time
  success: boolean;        // whether the last fire actually delivered to the target
}
