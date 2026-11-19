// A conversation's provider as a small lettered mark beside its name in the sidebar (as MonoCode and T3 Code show a
// provider per conversation; 2026-11-19). Letters, not the companies' logos. The model id decides: "claude-opus-5-5",
// "deepseek-v4.1-flash", "agy/gemini-4-argon" (OmniRoute's prefix is dropped).
const MARKS: [RegExp, string, string][] = [
  [/^(claude|opus|sonnet|haiku|fable)/, "C", "Claude"],
  [/^deepseek/, "D", "DeepSeek"],
  [/^(gpt|o\d|codex|chatgpt)/, "O", "OpenAI"],
  [/^gemini|^gemma/, "G", "Gemini"],
  [/^(mistral|devstral|codestral|magistral|ministral)/, "M", "Mistral"],
  [/^qwen/, "Q", "Qwen"],
  [/^(kimi|moonshot)/, "K", "Kimi"],
  [/^(glm|zai)/, "Z", "GLM"],
  [/^ernie/, "E", "ERNIE"],
  [/^grok/, "X", "Grok"],
  [/^(llama|meta)/, "L", "Llama"],
  [/^muse/, "M", "Muse"],
];

export function providerOf(model: string): { letter: string; name: string } | null {
  const id = model.toLowerCase().replace(/^[^/]*\//, "");
  if (!id) return null;
  const hit = MARKS.find(([re]) => re.test(id));
  return hit ? { letter: hit[1], name: hit[2] } : { letter: id.charAt(0).toUpperCase(), name: model };
}

export function ProviderMark({ model }: { model?: string }) {
  const p = model ? providerOf(model) : null;
  if (!p) return null;
  return (
    <span title={`${p.name} · ${model}`} aria-label={p.name}
      className="grid size-4 flex-none place-items-center rounded-[4px] border border-line bg-field text-[9.5px] font-semibold leading-none text-ink-2">{p.letter}</span>
  );
}
