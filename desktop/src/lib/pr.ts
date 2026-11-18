// A pull request as the core's {"op": "pr"} returns it (gh pr view, plus the inline review comments).
export type Pr = {
  none?: boolean; error?: string; branch?: string;
  number?: number; title?: string; state: string; url?: string; isDraft?: boolean; headRefName?: string; baseRefName?: string; reviewDecision?: string;
  statusCheckRollup?: { __typename?: string; name?: string; context?: string; status?: string; conclusion?: string; state?: string; detailsUrl?: string; targetUrl?: string }[];
  reviews?: { author?: { login: string }; authorAssociation?: string; state: string; body: string }[];
  comments?: { author?: { login: string }; authorAssociation?: string; body: string }[];
  inline?: { author?: string; association?: string; path: string; line?: number; body: string }[];
};

/** People with write access to the repository: only their comments go to the agent (anyone can comment on a public PR,
 * and a comment could carry instructions; security review). */
export const fromCollaborator = (association?: string) => ["OWNER", "MEMBER", "COLLABORATOR"].includes((association ?? "").toUpperCase());

/** A name from GitHub put in a message: one line, short. */
export const oneLine = (s: string, n = 80) => s.replace(/[\r\n]+/g, " ").slice(0, n);

export type Check = { name: string; state: "passed" | "failed" | "running" | "skipped"; url?: string };

/** gh's check runs and status contexts, each as passed, failed, running or skipped. */
export function checksOf(pr: Pr): Check[] {
  return (pr.statusCheckRollup ?? []).map((c) => {
    const verdict = (c.conclusion || c.state || "").toUpperCase();
    const state: Check["state"] = ["SUCCESS", "NEUTRAL"].includes(verdict) ? "passed"
      : ["FAILURE", "ERROR", "TIMED_OUT", "CANCELLED", "ACTION_REQUIRED", "STARTUP_FAILURE"].includes(verdict) ? "failed"
      : verdict === "SKIPPED" ? "skipped" : "running";
    return { name: c.name || c.context || "check", state, url: c.detailsUrl || c.targetUrl };
  });
}

/** What "Ask tazuna to open one" sends (the PR panel and the work panel). */
export const OPEN_PR = "Commit what is ready, push this branch and open a pull request with gh pr create: a short title, and a description of what changed and why.";

/** The header chip's text: "#12 ✓", "#12 ✗ 2", "#12 …", "#12 merged". */
export function prChip(pr: Pr): string | null {
  if (pr.none || pr.error || !pr.number) return null;
  if (pr.state === "MERGED") return `#${pr.number} merged`;
  if (pr.state === "CLOSED") return `#${pr.number} closed`;
  const cs = checksOf(pr), failed = cs.filter((c) => c.state === "failed").length;
  return `PR #${pr.number}${failed ? ` ✗ ${failed}` : cs.some((c) => c.state === "running") ? " …" : cs.length ? " ✓" : ""}`;
}
