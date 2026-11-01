//! Aurora: one calm column, indigo accents, a status bar at the bottom.

use super::*;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    widgets::{Block as Panel, BorderType},
};

const OPTS: Opts = Opts { marker: "◆", prompt: PromptStyle::Bar, group_tools: false, timeline: false, durations: false };

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let ch = composer_height(v, f.area().width.saturating_sub(2));
    let [header, _, body, composer, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Min(3), Constraint::Length(ch), Constraint::Length(1)])
            .areas(f.area());

    let left = Line::from(vec![
        Span::styled(" ◆ tazuna ", Style::new().fg(th.bg).bg(th.accent).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(model_short(&s.model), st(th.text)),
        Span::styled(" · ", st(th.faint)),
        Span::styled(s.effort.clone(), st(th.warn)),
    ]);
    let dir = s.cwd.rsplit('/').next().unwrap_or_default().to_string();
    split_row(f, header, left, Line::from(vec![Span::styled("aurora · ", st(th.faint)), Span::styled(format!("{dir} "), st(th.dim))]), None);

    let area = body.inner(Margin { horizontal: 2, vertical: 0 });
    let (lines, last_rows) = transcript(s, v, th, &OPTS, area.width as usize);
    let last_block = render_transcript(f, area, lines, last_rows, v.scroll);

    let b = Panel::bordered().border_type(BorderType::Rounded).border_style(st(th.border));
    let hint = composer_rows(v, th, "›", "Message tazuna…", composer.width.saturating_sub(4));
    f.render_widget(Paragraph::new(hint).block(b), composer.inner(Margin { horizontal: 1, vertical: 0 }));

    let (glyph, label, color) = activity(s, v, th);
    let u = &s.usage;
    let sep = || Span::styled("  │  ", st(th.faint));
    let left = Line::from(vec![
        Span::styled(format!(" {glyph} "), st(color).add_modifier(Modifier::BOLD)),
        Span::styled(label, st(color)),
        sep(),
        Span::styled(format!("turn {}", s.turns), st(th.dim)),
        sep(),
        Span::styled(format!("↑ {}  ↓ {}", human(u.total_in()), human(u.output)), st(th.text)),
        sep(),
        Span::styled(format!("cache {:.0}%", u.cache_hit() * 100.0), st(th.dim)),
        sep(),
        Span::styled(cost_str(s), st(th.ok)),
    ]);
    let mut left = left;
    if let Some((t, color)) = limits_str(s, th) {
        left.spans.extend([sep(), Span::styled(t, st(color))]);
    }
    let play = if v.paused { "⏸ paused".to_string() } else { format!("▶ {:.1}×", v.speed) };
    let right = Line::from(vec![Span::styled(play, st(th.dim)), Span::styled(format!("  {} ", clock_str(v.clock_ms)), st(th.text))]);
    split_row(f, status, left, right, Some(th.surface));

    let mut pulse = status;
    pulse.width = pulse.width.min(28);
    Areas { pulse, last_block, transcript: area }
}
