//! Timeline: neon on black. Every step on a time gutter, turn dividers with token and cost deltas, tool durations.

use super::*;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    widgets::{Block as Panel, BorderType},
};

const OPTS: Opts = Opts { marker: "", prompt: PromptStyle::Bar, group_tools: false, timeline: true, durations: true };

fn seg(text: String, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(format!(" {text} "), Style::new().fg(fg).bg(bg).add_modifier(Modifier::BOLD))
}

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let ch = composer_height(v, f.area().width);
    let [header, _, body, composer, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3), Constraint::Length(ch), Constraint::Length(1)])
            .areas(f.area());

    let left = Line::from(vec![
        seg("TAZUNA".into(), th.bg, th.accent),
        seg(model_short(&s.model), th.text, th.surface),
        seg(s.effort.to_uppercase(), th.bg, th.accent2),
    ]);
    let right = Line::from(vec![Span::styled("timeline ", st(th.faint))]);
    split_row(f, header, left, right, None);

    let area = body.inner(Margin { horizontal: 1, vertical: 0 });
    let (lines, last_rows) = transcript(s, v, th, &OPTS, area.width as usize);
    let last_block = render_transcript(f, area, lines, last_rows, v.scroll);

    let b = Panel::bordered().border_type(BorderType::Rounded).border_style(st(th.accent2).add_modifier(Modifier::DIM));
    let hint = composer_rows(v, th, "❯", "Message tazuna…", composer.width.saturating_sub(2));
    f.render_widget(Paragraph::new(hint).block(b), composer);

    let (glyph, label, color) = activity(s, v, th);
    let u = &s.usage;
    let ctx = s.context as f64 / s.context_window as f64 * 100.0;
    let act = seg(format!("{glyph} {}", label.to_uppercase()), th.bg, color);
    let act_w = act.width() as u16;
    let left = Line::from(vec![
        act,
        seg(format!("T{}", s.turns), th.text, th.border),
        seg(format!("↑{} ↓{}", human(u.total_in()), human(u.output)), th.text, th.surface),
        seg(format!("CTX {ctx:.1}%"), th.text, th.border),
        seg(format!("CACHE {:.0}%", u.cache_hit() * 100.0), th.text, th.surface),
        seg(cost_str(s), th.bg, th.ok),
    ]);
    let mut left = left;
    if let Some((text, color)) = limits_str(s, th) {
        left.spans.push(seg(text, color, th.surface));
    }
    let play = if v.paused { "⏸".to_string() } else { format!("▶ {:.1}×", v.speed) };
    let right = Line::from(vec![Span::styled(format!("{play}  "), st(th.dim)), seg(times_str(s, v), th.bg, th.accent)]);
    split_row(f, status, left, right, Some(th.bg));

    let mut pulse = status;
    pulse.width = act_w;
    Areas { pulse, last_block, transcript: area }
}
