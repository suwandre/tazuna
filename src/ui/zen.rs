//! Zen: a centered, borderless column. Tool runs fold into one line; numbers stay in one quiet row.

use super::*;
use ratatui::layout::{Constraint, Layout};

pub(super) const OPTS: Opts = Opts { marker: "", prompt: PromptStyle::Quote, group_tools: true, timeline: false, durations: false };
const MAX_WIDTH: u16 = 92;

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let full = f.area();
    let w = full.width.saturating_sub(4).min(MAX_WIDTH);
    let col = Rect { x: full.x + (full.width - w) / 2, width: w, ..full };

    // one quiet row: the model, the cost, then the plan's windows and SubRouter's balance where they fit beside them;
    // the two clocks went (the status at the bottom says what runs), and the plan wrapped onto a second row at 120
    // columns (design review 9). A row that does not fit puts the plan under it, as before.
    let gap = || Span::raw("   ");
    let mut right = vec![Span::styled(model_short(&s.model), st(th.dim)), Span::styled(format!(" {}", s.effort), st(th.faint)), gap(),
        Span::styled(cost_str(s), st(th.ink2))];
    let mut extra: Vec<Span> = plan_spans(s, th).map(|p| [vec![Span::styled("Claude ", st(th.dim))], p].concat()).unwrap_or_default();
    if let Some(b) = balance_str(s) {
        extra.extend([Span::raw(if extra.is_empty() { "" } else { "   " }), Span::styled(b, st(th.dim))]);
    }
    let one_row = Line::from([right.clone(), vec![gap()], extra.clone()].concat()).width() + "tazuna".len() + 4 <= w as usize;
    if one_row && !extra.is_empty() {
        right.push(gap());
        right.append(&mut extra);
    }
    let [top, under, body, _, rule, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(if one_row || extra.is_empty() { 0 } else { 1 }),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(col);
    split_row(f, top, Line::from(Span::styled("tazuna", st(th.faint).add_modifier(Modifier::ITALIC))), Line::from(right), None);
    if !one_row && !extra.is_empty() {
        split_row(f, under, Line::default(), Line::from(extra), None);
    }

    let (last_block, trows, top_line) = render_transcript(f, body, transcript(s, v, th, &OPTS, body.width as usize), v.scroll);

    f.render_widget(Paragraph::new(Span::styled("─".repeat(rule.width as usize), st(th.border))), rule);
    let (glyph, label, color) = activity(s, v, th);
    // a permission question takes the row, as in the other designs' message box (zen drew none: the run just stood still)
    let left = if v.asking.is_some() {
        composer_rows(v, th, "›", "", input.width.saturating_sub(12)).remove(0) // less the status on the right
    } else if v.live && !v.input.is_empty() {
        composer_line(v, th, "›", "", input.width / 2)
    } else {
        Line::from(vec![Span::styled("› ", st(th.accent)), Span::styled("say something…", st(th.faint).add_modifier(Modifier::ITALIC))])
    };
    // a note for a moment ("copied 12 chars", the design ctrl-t switched to): zen has no row under its input for it
    let note = v.toast.as_ref().map_or(String::new(), |(t, _)| format!("{t}   "));
    let right = Line::from(vec![
        Span::styled(note, st(th.ok)),
        Span::styled(format!("{glyph} {label}"), st(color)),
        Span::styled(if v.live { String::new() } else { format!("   {}", play_str(v, "paused")) }, st(th.faint)),
        Span::styled(if v.live { "" } else { "   1-4 · e · q" }, st(th.faint)),
    ]);
    split_row(f, input, left, right, None);

    Areas { pulse: top, last_block, transcript: body, rows: trows, links: vec![], files: None, top_line, ..Default::default() }
}
