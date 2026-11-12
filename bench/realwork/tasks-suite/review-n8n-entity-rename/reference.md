Known defect (a regression the follow-up PR #43220 fixed): the PR renames the entity `AgentFile` to `KnowledgeFile` while
its table stays `agent_files`. `n8n export:entities` names each archive file after the entity class
(`agentfile.jsonl`), and `n8n import:entities` matches files to entities by that name, skipping an unmatched file
with only a warning and still reporting success. So an archive exported before the rename (`agentfile.jsonl`) is
silently dropped on import into an instance after it: data loss without an error.
A review that sees the export/import naming break, or asks for a migration path for existing archives, has found it.
Other valid findings count too.
