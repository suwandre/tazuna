//! Cockpit: transcript plus a live instrument panel (context, cache, tokens per turn, cost, tools, files).

use super::*;
use crate::session::Block;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    widgets::{Block as Panel, BorderType, LineGauge, Sparkline},
};

const OPTS: Opts = Opts { marker: "▸", prompt: PromptStyle::Bar, group_tools: false, timeline: false, durations: true };
const SIDEBAR: u16 = 38;

fn panel<'a>(title: &'a str, th: &Theme) -> Panel<'a> {
    Panel::bordered()
        .border_type(BorderType::Rounded)
        .border_style(st(th.border))
        .title(Line::from(vec![Span::raw(" "), Span::styled(title, st(th.dim).add_modifier(Modifier::BOLD)), Span::raw(" ")]))
}

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let ch = composer_height(v, f.area().width);
    let [header, body, composer] = Layout::vertical([Constraint::Length(1), Constraint::Min(10), Constraint::Length(ch)]).areas(f.area());
    let (glyph, label, color) = activity(s, v, th);

    let left = Line::from(vec![
        Span::styled(" ▣ TAZUNA ", Style::new().fg(th.bg).bg(th.accent).add_modifier(Modifier::BOLD)),
        Span::styled(" cockpit", st(th.faint)),
    ]);
    let right = Line::from(vec![
        Span::styled(format!("{glyph} {label}"), st(color)),
        Span::styled("   ", st(th.faint)),
        Span::styled(model_short(&s.model), st(th.text)),
        Span::styled(format!(" · {} ", s.effort), st(th.warn)),
    ]);
    split_row(f, header, left, right, Some(th.surface));

    let [main, side] = Layout::horizontal([Constraint::Min(40), Constraint::Length(SIDEBAR)]).areas(body);
    let frame = panel("transcript", th);
    let inner = frame.inner(main).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(frame, main);
    let (lines, last_rows) = transcript(s, v, th, &OPTS, inner.width as usize);
    let last_block = render_transcript(f, inner, lines, last_rows, v.scroll);

    let files = files_touched(s);
    let tools = tool_tally(s);
    let [p_session, p_context, p_tokens, p_cost, p_tools, p_files] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(4),
        Constraint::Length(6),
        Constraint::Length(4),
        Constraint::Length(tools.len().max(1) as u16 + 3),
        Constraint::Min(3),
    ])
    .areas(side);

    // session
    let b = panel("session", th);
    let a = b.inner(p_session).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_session);
    let play = if v.paused { "paused".to_string() } else { format!("{:.1}×", v.speed) };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![Span::styled(format!("{glyph} "), st(color)), Span::styled(label, st(color).add_modifier(Modifier::BOLD))]),
            Line::from(vec![kv("elapsed", &clock_str(v.clock_ms), th), Span::raw("  "), kv("turn", &s.turns.to_string(), th)]),
            Line::from(vec![if v.live { kv("mode", &format!("live · {} · {}", if s.profile.is_empty() { "lean" } else { &s.profile }, s.mode), th) } else { kv("replay", &play, th) }]),
        ]),
        a,
    );

    // context + cache gauges
    let b = panel("context", th);
    let a = b.inner(p_context).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_context);
    let [g1, g2] = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(a);
    let ctx = s.context as f64 / s.context_window as f64;
    gauge(f, g1, ctx, &format!("ctx {:>6} / {}", human(s.context), human(s.context_window)), th.accent, th);
    let misses = s.cache_misses();
    let miss = match misses {
        0 => String::new(),
        1 => " · 1 miss".to_string(),
        n => format!(" · {n} misses"),
    };
    gauge(f, g2, s.usage.cache_hit(), &format!("cache {:>4.0}%{miss}", s.usage.cache_hit() * 100.0), if misses == 0 { th.ok } else { th.warn }, th);

    // tokens per turn
    let b = panel("tokens", th);
    let a = b.inner(p_tokens).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_tokens);
    let [t1, spark] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(a);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("↑ ", st(th.faint)),
            Span::styled(human(s.usage.total_in()), st(th.text)),
            Span::styled("   ↓ ", st(th.faint)),
            Span::styled(human(s.usage.output), st(th.text)),
            Span::styled("   out/turn", st(th.faint)),
        ])),
        t1,
    );
    let data: Vec<u64> = s.turn_usage.iter().map(|u| u.output).collect();
    let tail = &data[data.len().saturating_sub(spark.width as usize)..];
    f.render_widget(Sparkline::default().data(tail.iter().copied()).style(st(th.accent2)), spark);

    // cost
    let b = panel("cost", th);
    let a = b.inner(p_cost).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_cost);
    let per_turn = if s.turns > 0 { s.cost() / s.turns as f64 } else { 0.0 };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![Span::styled(cost_str(s), st(th.ok).add_modifier(Modifier::BOLD)), Span::styled(format!("   ${per_turn:.3}/turn"), st(th.dim))]),
            Line::from(Span::styled(if s.exact_cost.is_some() { "exact (reported)" } else { "estimate (list prices)" }, st(th.faint))),
        ]),
        a,
    );

    // tools
    let b = panel("tools", th);
    let a = b.inner(p_tools).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_tools);
    let mut rows: Vec<Line> = tools
        .iter()
        .map(|(n, c, err, secs)| {
            Line::from(vec![
                Span::styled(format!("{n:<7}"), st(th.accent2).add_modifier(Modifier::BOLD)),
                Span::styled(format!("×{c:<3}"), st(th.text)),
                Span::styled(if *err > 0 { format!(" {err} failed") } else { " ✓".into() }, st(if *err > 0 { th.err } else { th.ok })),
                Span::styled(format!("  {secs:.1}s"), st(th.faint)),
            ])
        })
        .collect();
    if rows.is_empty() {
        rows.push(Line::from(Span::styled("none yet", st(th.faint))));
    }
    f.render_widget(Paragraph::new(rows), a);

    // files
    let b = panel("files", th);
    let a = b.inner(p_files).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_files);
    let rows: Vec<Line> = if files.is_empty() {
        vec![Line::from(Span::styled("none yet", st(th.faint)))]
    } else {
        files.iter().take(a.height as usize).map(|p| Line::from(Span::styled(fit_path(p, a.width as usize), st(th.dim)))).collect()
    };
    f.render_widget(Paragraph::new(rows), a);

    let b = Panel::bordered().border_type(BorderType::Rounded).border_style(st(th.border));
    let hint = composer_rows(v, th, "›", "Message tazuna…", composer.width.saturating_sub(2));
    f.render_widget(Paragraph::new(hint).block(b), composer);

    Areas { pulse: p_tools, last_block, transcript: inner }
}

fn kv(k: &str, v: &str, th: &Theme) -> Span<'static> {
    Span::styled(format!("{k} {v}"), st(th.text))
}

fn gauge(f: &mut Frame, area: Rect, ratio: f64, label: &str, color: Color, th: &Theme) {
    f.render_widget(
        LineGauge::default()
            .ratio(ratio.clamp(0.0, 1.0))
            .label(Span::styled(label.to_string(), st(th.text)))
            .filled_style(st(color))
            .unfilled_style(st(th.border)),
        area,
    );
}

/// (tool, calls, failures, total seconds), in first-use order.
fn tool_tally(s: &Session) -> Vec<(String, usize, usize, f64)> {
    let mut out: Vec<(String, usize, usize, f64)> = Vec::new();
    for b in &s.blocks {
        let Block::Tool(t) = b else { continue };
        let n = short_name(&t.name).to_string();
        let i = match out.iter().position(|r| r.0 == n) {
            Some(i) => i,
            None => {
                out.push((n, 0, 0, 0.0));
                out.len() - 1
            }
        };
        out[i].1 += 1;
        out[i].2 += t.error as usize;
        out[i].3 += t.secs().unwrap_or(0.0);
    }
    out
}

/// Shortens a path from the left so the file name stays visible.
fn fit_path(p: &str, width: usize) -> String {
    let n = p.chars().count();
    if n <= width { p.to_string() } else { format!("…{}", p.chars().skip(n + 1 - width).collect::<String>()) }
}
