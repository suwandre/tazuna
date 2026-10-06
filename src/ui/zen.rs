//! Zen: a centered, borderless column in a warm muted palette. Tool runs fold into one line; numbers stay in one quiet row.

use super::*;
use ratatui::layout::{Constraint, Layout};

const OPTS: Opts = Opts { marker: "", prompt: PromptStyle::Quote, group_tools: true, timeline: false, durations: false };
const MAX_WIDTH: u16 = 92;

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let full = f.area();
    let w = full.width.saturating_sub(4).min(MAX_WIDTH);
    let col = Rect { x: full.x + (full.width - w) / 2, width: w, ..full };
    let [top, _, body, _, rule, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(col);

    let u = &s.usage;
    let dot = || Span::styled("  ·  ", st(th.faint));
    let right = Line::from(vec![
        Span::styled(model_short(&s.model), st(th.dim)),
        Span::styled(format!(" {}", s.effort), st(th.faint)),
        dot(),
        Span::styled(clock_str(v.clock_ms), st(th.dim)),
        dot(),
        Span::styled(format!("{} ↑  {} ↓", human(u.total_in()), human(u.output)), st(th.dim)),
        dot(),
        Span::styled(cost_str(s), st(th.accent)),
    ]);
    split_row(f, top, Line::from(Span::styled("tazuna", st(th.faint).add_modifier(Modifier::ITALIC))), right, None);

    let (lines, last_rows) = transcript(s, v, th, &OPTS, body.width as usize);
    let last_block = render_transcript(f, body, lines, last_rows, v.scroll);

    f.render_widget(Paragraph::new(Span::styled("─".repeat(rule.width as usize), st(th.border))), rule);
    let (glyph, label, color) = activity(s, v, th);
    let left = if v.live && !v.input.is_empty() {
        composer_line(v, th, "›", "", input.width / 2)
    } else {
        Line::from(vec![Span::styled("› ", st(th.accent)), Span::styled("say something…", st(th.faint).add_modifier(Modifier::ITALIC))])
    };
    let right = Line::from(vec![
        Span::styled(format!("{glyph} {label}"), st(color)),
        Span::styled(if v.paused { "   paused".to_string() } else { format!("   {:.1}×", v.speed) }, st(th.faint)),
        Span::styled("   1-4 · e · q", st(th.faint)),
    ]);
    split_row(f, input, left, right, None);

    Areas { pulse: top, last_block }
}
