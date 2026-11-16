// tazuna's own parts, registered in the window's spots the same way a plugin registers (docs/PLUGINS.md): a plugin can
// replace any of them by registering its id, or hide it. Imported once by main.tsx.
import { AttachButton, EffortPill, ModelPill, ModePill } from "./components/Composer";
import { Card, Timeline, builtInWidgets } from "./components/Rail";
import { NewChatButton, OpenFolderButton, SettingsRow } from "./components/Sidebar";
import { Step } from "./components/Step";
import { MessageChange, MessageMeta, UserMessage } from "./components/Transcript";
import { Markdown } from "./lib/markdown";
import { slots } from "./lib/slots";
import type { ToolStep, Usage } from "./lib/types";

const own = "tazuna";

builtInWidgets.forEach((w, i) => slots.register({ id: `rail.${w.id}`, slot: "rail", order: i * 10, source: own,
  render: ({ usage }: { usage: Usage }) => { const body = w.render(usage); return body == null ? null : <Card title={w.title}>{body}</Card>; } }));

slots.register({ id: "rail.timeline", slot: "rail", order: 50, source: own, render: Timeline });

slots.register({ id: "composer.mode", slot: "composer.pills", order: 10, source: own, render: ModePill });
slots.register({ id: "composer.model", slot: "composer.pills", order: 20, source: own, render: ModelPill });
slots.register({ id: "composer.effort", slot: "composer.pills", order: 30, source: own, render: EffortPill });
slots.register({ id: "composer.attach", slot: "composer.pills", order: 40, source: own, render: AttachButton });

slots.register({ id: "sidebar.new-chat", slot: "sidebar.top", order: 10, source: own, render: NewChatButton });
slots.register({ id: "sidebar.open-folder", slot: "sidebar.top", order: 20, source: own, render: OpenFolderButton });
slots.register({ id: "sidebar.settings", slot: "sidebar.footer", order: 10, source: own, render: SettingsRow });

slots.register({ id: "message.user.default", slot: "message.user", order: 10, source: own, render: UserMessage });
slots.register({ id: "message.text.markdown", slot: "message.text", order: 10, source: own, render: ({ text }: { text: string }) => <Markdown text={text} /> });
slots.register({ id: "message.change", slot: "message.after", order: 10, source: own, render: MessageChange });
slots.register({ id: "message.meta", slot: "message.after", order: 20, source: own, render: MessageMeta });
slots.register({ id: "tool.view.default", slot: "tool.view", order: 10, source: own, render: ({ step }: { step: ToolStep }) => <Step t={step} /> });
