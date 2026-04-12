export type SessionStatus = "running" | "waiting" | "idle";

export interface SessionState {
  id: string;           // session uuid — filename without .jsonl
  project: string;      // last segment of cwd from first JSONL line
  cwd: string;          // full cwd path
  status: SessionStatus;
  last_output: string;  // last assistant message content, max 500 chars; "Running: {tool_name}" for tool_use
  tool_count: number;   // count of tool_use events in file
  recent_tools: string[]; // last 5 tool names used in session
  started_at: string;   // timestamp of first event
  modified_at: string;  // file last modified timestamp
}
