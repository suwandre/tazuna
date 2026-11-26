//! Zen: a centered, borderless column. Tool runs fold into one line; numbers stay in one quiet row.

use super::*;
use ratatui::layout::{Constraint, Layout};

pub(super) const OPTS: Opts = Opts { group_tools: true, durations: false };
pub(super) const MAX_WIDTH: u16 = 92;

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let full = f.area();
    let w = full.width.saturating_sub(4).min(MAX_WIDTH);
    let col = Rect { x: full.x + (full.width - w) / 2, width: w, ..full };

    // one quiet row: the model, the cost, then the plan's windows and SubRouter's balance where they fit beside them;
    // the two clocks went (the status at the bottom says what runs), and the plan wrapped onto a second row at 120
    // columns (design review 9). A row that does not fit puts the plan under it, as before.
    let gap = || Span::raw("   ");
    let mut right = vec![Span::styled(if !v.live && s.model.is_empty() { "recorded session".into() } else { model_short(&s.model) }, st(th.dim)), Span::styled(format!(" {}", s.effort), st(th.faint)), gap(),
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
    if !v.live {
        let [top, under, metadata, _, body, footer] = Layout::vertical([
            Constraint::Length(1), Constraint::Length(u16::from(!one_row && !extra.is_empty())),
            Constraint::Length(1), Constraint::Length(1), Constraint::Min(3), Constraint::Length(2),
        ]).areas(col);
        split_row(f, top, Line::from(Span::styled("tazuna", st(th.faint).add_modifier(Modifier::ITALIC))), Line::from(right), None);
        if !one_row && !extra.is_empty() { split_row(f, under, Line::default(), Line::from(extra), None); }
        draw_replay_metadata(f, metadata, v, th);
        let (last_block, trows, top_line) = render_transcript(f, body, transcript(s, v, th, &OPTS, body.width as usize), v.scroll, th);
        draw_replay_footer(f, footer, v, th);
        return Areas { pulse: top, last_block, transcript: body, rows: trows, top_line, ..Default::default() };
    }
    // the message box under the rule: the status on its right, and on its left the question, the draft on as many rows
    // as it takes (six at most, then it scrolls) with the `@` and `/` completions under it, or the placeholder. It was one
    // row that showed the end of a draft and no completions (design review 11).
    let (glyph, label, color) = activity(s, v, th);
    // while a reply runs: the state in short and the key that stops it; its time, output and step are on the working row
    // at the transcript's end (design review 13; "⠹ Working 12 s" stood here since review 11). A retry's wait and a
    // question keep their own words.
    let busy = working(s, v);
    let said = vec![Span::styled(format!("{glyph} {}", if busy { "working" } else { &label }), st(color)), Span::styled(if busy { esc_hint(v) } else { "" }, st(th.faint))];
    let typing = v.live && !v.input.is_empty() && v.asking.is_none();
    // a note for a moment ("copied 12 chars", the design ctrl-t switched to); while typing it is the row under the draft
    // (and under a question it stands in for the answers' keys, composer_rows_in)
    let note = if typing || (v.asking.is_some() && v.input.is_empty()) { String::new() } else { v.toast.as_ref().map_or(String::new(), |(t, _)| format!("{t}   ")) };
    // a question in a narrow column has the row to itself (its "?" says who is waited for): beside "● needs you" the
    // answers were cut to "a al" at 40 columns
    let alone = v.asking.is_some() && col.width < 60;
    // the text starts one column left of the column, so the prompt's `›` (after a space, as composer_rows draws it) stands
    // on the column's edge with or without a draft
    let shift = u16::from(col.x > full.x);
    let rest = [said, vec![
        Span::styled(if v.live { String::new() } else { format!("   {}", play_str(v, "paused")) }, st(th.faint)),
        Span::styled(if v.live { "" } else { "   tab · e · q" }, st(th.faint)),
    ]].concat();
    // the permission mode as a pill, always, auto too (design review 13: auto showed nothing, beside a question nothing but
    // full did). Where it would leave a draft under 30 columns (9 at 60 columns), it shrinks: full to its dot and "full",
    // the others to the dot.
    let room = |pill: &[Span]| ((col.width + shift) as usize).saturating_sub(note.width() + Line::from(pill.to_vec()).width() + Line::from(rest.clone()).width() + 4);
    let mut pill = if v.live { mode_pill(&v.mode, th, false) } else { vec![] };
    if typing && room(&pill) < 30 {
        pill = mode_pill(&v.mode, th, true);
    }
    pill.push(Span::raw(if v.live { "  " } else { "" }));
    let status = Line::from(if alone { vec![] } else { [vec![Span::styled(note, st(th.ok))], pill, rest].concat() });
    let text_w = (col.width + shift).saturating_sub(if alone { 0 } else { status.width() as u16 + 2 });
    // as many rows of the draft as leave the header, three rows of transcript and the completions' row on screen
    let fixed = 8 + u16::from(!(one_row || extra.is_empty()));
    let rows = if typing || v.asking.is_some() {
        composer_rows_in(v, th, "›", "", text_w, (col.height.saturating_sub(fixed) as usize).clamp(1, 6))
    } else {
        vec![Line::from(vec![Span::styled(" › ", st(th.accent)), Span::styled("say something…", st(th.faint).add_modifier(Modifier::ITALIC))])]
    };
    let [top, under, _, body, _, rule, input] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(if one_row || extra.is_empty() { 0 } else { 1 }),
        Constraint::Length(1), // air under the header: the transcript's first line touched it
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(rows.len() as u16),
    ])
    .areas(col);
    let text = Rect { x: input.x - shift, width: text_w, ..input };
    split_row(f, top, Line::from(Span::styled("tazuna", st(th.faint).add_modifier(Modifier::ITALIC))), Line::from(right), None);
    if !one_row && !extra.is_empty() {
        split_row(f, under, Line::default(), Line::from(extra), None);
    }

    let (last_block, trows, top_line) = render_transcript(f, body, transcript(s, v, th, &OPTS, body.width as usize), v.scroll, th);

    f.render_widget(Paragraph::new(Span::styled("─".repeat(rule.width as usize), st(th.border))), rule);
    f.render_widget(Paragraph::new(rows), text);
    split_row(f, Rect { y: input.bottom().saturating_sub(1), height: 1, ..input }, Line::default(), status, None);

    Areas { pulse: top, last_block, transcript: body, rows: trows, links: vec![], files: None, top_line, composer: typing.then_some(text), ..Default::default() }
}
