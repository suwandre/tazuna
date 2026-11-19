// tazuna's own parts, registered in the window's spots the same way a plugin registers (docs/PLUGINS.md): a plugin can
// replace any of them by registering its id, or hide it. Imported once by main.tsx.
// EffortPill (a pill of its own until desktop 0.1.70) stays exported for a plugin that wants it back
import { AttachButton, ModelPill, ModePill, WherePill } from "./components/Composer";
import { ChangesSection, ChecksSection, PrSection, Timeline } from "./components/Rail";
import { NewChatButton, OpenFolderButton, SettingsRow } from "./components/Sidebar";
import { Step } from "./components/Step";
import { MessageChange, MessageFailure, MessageImages, MessageMeta, UserMessage } from "./components/Transcript";
import { Markdown } from "./lib/markdown";
import { slots } from "./lib/slots";
import type { ToolStep } from "./lib/types";

const own = "tazuna";

slots.register({ id: "rail.changes", slot: "rail", order: 10, source: own, render: ChangesSection });
slots.register({ id: "rail.checks", slot: "rail", order: 20, source: own, render: ChecksSection });
slots.register({ id: "rail.pr", slot: "rail", order: 30, source: own, render: PrSection });

slots.register({ id: "rail.timeline", slot: "rail", order: 50, source: own, render: Timeline });

slots.register({ id: "composer.where", slot: "composer.pills", order: 5, source: own, render: WherePill });
slots.register({ id: "composer.mode", slot: "composer.pills", order: 10, source: own, render: ModePill });
slots.register({ id: "composer.model", slot: "composer.pills", order: 20, source: own, render: ModelPill });
slots.register({ id: "composer.attach", slot: "composer.pills", order: 40, source: own, render: AttachButton });

slots.register({ id: "sidebar.new-chat", slot: "sidebar.top", order: 10, source: own, render: NewChatButton });
slots.register({ id: "sidebar.open-folder", slot: "sidebar.top", order: 20, source: own, render: OpenFolderButton });
slots.register({ id: "sidebar.settings", slot: "sidebar.footer", order: 10, source: own, render: SettingsRow });

slots.register({ id: "message.user.default", slot: "message.user", order: 10, source: own, render: UserMessage });
slots.register({ id: "message.text.markdown", slot: "message.text", order: 10, source: own, render: ({ text }: { text: string }) => <Markdown text={text} /> });
slots.register({ id: "message.failure", slot: "message.after", order: 5, source: own, render: MessageFailure });
slots.register({ id: "message.images", slot: "message.after", order: 8, source: own, render: MessageImages });
slots.register({ id: "message.change", slot: "message.after", order: 10, source: own, render: MessageChange });
slots.register({ id: "message.meta", slot: "message.after", order: 20, source: own, render: MessageMeta });
slots.register({ id: "tool.view.default", slot: "tool.view", order: 10, source: own, render: ({ step }: { step: ToolStep }) => <Step t={step} /> });
