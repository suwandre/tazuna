//! Cockpit: transcript plus a live instrument panel (context, cache, tokens per turn, cost, tools, files).

use super::*;
use crate::session::Block;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    widgets::{Block as Panel, BorderType, LineGauge, Sparkline},
};

pub(super) const OPTS: Opts = Opts { marker: "▸", prompt: PromptStyle::Bar, group_tools: false, timeline: false, durations: true };
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
    let (last_block, trows) = render_transcript(f, inner, transcript(s, v, th, &OPTS, inner.width as usize), v.scroll);

    let files = files_touched(s);
    let tools = tool_tally(s);
    let [p_session, p_context, p_tokens, p_cost, p_tools, p_files] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(5),
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
            // elapsed: the whole session, the user's own time too; working: message to last token, summed
            Line::from(vec![kv("elapsed", &clock_str(v.clock_ms - v.since_ms), th), Span::raw("  "), kv("working", &clock_str(s.busy(v.clock_ms)), th)]),
            Line::from(vec![kv("turn", &s.turns.to_string(), th), Span::raw("  "),
                if v.live { Span::styled(format!("live · {} · {}", if s.profile.is_empty() { "lean" } else { &s.profile }, s.mode), st(th.text)) } else { kv("replay", &play, th) }]),
        ]),
        a,
    );

    // context + cache gauges
    let b = panel("context", th);
    let a = b.inner(p_context).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_context);
    let [g1, g2, g3, g4] = Layout::vertical([Constraint::Length(1); 4]).areas(a);
    // the window as the model reports it: none until claude's first result, none on the own loop
    let (ctx, window) = s.context_window.map_or((0.0, String::new()), |w| (s.context as f64 / w as f64, format!(" / {}", human(w))));
    gauge(f, g1, ctx, &format!("{:<12}{}{window}", "ctx", human(s.context)), th.accent, th);
    let misses = s.cache_misses();
    let miss = match misses {
        0 => String::new(),
        1 => " · 1 miss".to_string(),
        n => format!(" · {n} misses"),
    };
    // of everything sent, the share read back from claude's cache (a tenth of the price); what went into the cache is
    // its own line: written once at a premium, it is most of what a turn adds
    // read back from claude's cache (a tenth of the price; its share of all sent) and written into it (new content,
    // once, at a premium)
    // the session's share, and the last call's (what Claude Code's cache meters show: one call after a short exchange
    // reads ~99%, the whole session less, since every call writes what is new once)
    let last = s.last_call_hit().map_or(String::new(), |h| format!(" · last {:.0}%", h * 100.0));
    gauge(f, g2, s.usage.cache_hit(), &format!("{:<12}{} {:.0}%{last}", "cache read", human(s.usage.cache_read), s.usage.cache_hit() * 100.0), if misses == 0 { th.ok } else { th.warn }, th);
    f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!("{:<12}", "cache write"), st(th.text)), Span::styled(human(s.usage.cache_write), st(th.text)), Span::styled(miss, st(th.warn))])), g3);
    // how long the cache stays warm: after that the next call writes the whole context again
    if let (Some(left), Some(ttl)) = (s.cache_left(v.clock_ms), s.cache_ttl_ms) {
        let life = if ttl >= 3_600_000.0 { "1h" } else { "5m" };
        let (text, color) = if left > 0.0 { (format!("{} of {life}", clock_str(left)), th.ok) } else { (format!("expired ({life})"), th.warn) };
        f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!("{:<12}", "cache life"), st(th.text)), Span::styled(text, st(color))])), g4);
    }

    // tokens per turn
    let b = panel("tokens", th);
    let a = b.inner(p_tokens).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_tokens);
    let [t1, spark, caption, pace] = Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)]).areas(a);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            // sent: every call's whole input added up (each call sends the conversation again), not the context's size
            Span::styled("↑ ", st(th.faint)),
            Span::styled(human(s.usage.total_in()), st(th.text)),
            Span::styled(" sent  ↓ ", st(th.faint)),
            Span::styled(human(s.usage.output), st(th.text)),
            Span::styled(" out", st(th.faint)),
        ])),
        t1,
    );
    f.render_widget(Paragraph::new(Span::styled(format!("output per call ({})", s.turn_usage.len()), st(th.faint))), caption);
    // the call in flight, live: the wait for its first token, then how fast it writes (≈: its output is estimated until
    // it ends); between calls the last one's exact figures
    let secs = |ms: f64| format!("{:.1}s", ms / 1000.0);
    let pace_line = match (s.in_flight(), s.calls.last()) {
        (Some((start, None, _)), _) => vec![Span::styled("waiting for first token · ", st(th.faint)), Span::styled(secs(v.clock_ms - start), st(th.text))],
        (Some((start, Some(first), est)), _) => {
            let rate = if v.clock_ms - first > 500.0 { format!("≈{:.0}", est / (v.clock_ms - first) * 1000.0) } else { "…".into() };
            vec![Span::styled(rate, st(th.text)), Span::styled(" tok/s · ", st(th.faint)), Span::styled(secs(first - start), st(th.text)), Span::styled(" to first token", st(th.faint))]
        }
        (None, Some(c)) => {
            let mut l = vec![];
            if c.gen_ms > 0.0 {
                l.extend([Span::styled(format!("{:.0}", c.out as f64 / c.gen_ms * 1000.0), st(th.text)), Span::styled(" tok/s", st(th.faint))]);
            }
            if let Some(t) = c.ttft_ms {
                l.extend([Span::styled(if l.is_empty() { "" } else { " · " }, st(th.faint)), Span::styled(secs(t), st(th.text)), Span::styled(" to first token", st(th.faint))]);
            }
            l
        }
        (None, None) => vec![],
    };
    f.render_widget(Paragraph::new(Line::from(pace_line)), pace);
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
            Line::from(vec![Span::styled(cost_str(s), st(th.ok).add_modifier(Modifier::BOLD)), Span::styled(format!("   {}/turn", usd(per_turn)), st(th.dim))]),
            // the plan's windows and SubRouter's balance on lines of their own: on one line the sidebar cut the balance
            // to "$8" (2026-11-02)
            // each account named in its own colour, its numbers beside it
            match plan_str(s, th) {
                Some((t, color)) => Line::from(vec![Span::styled("Claude", st(th.warn).add_modifier(Modifier::BOLD)), Span::styled(t.trim_start_matches("Claude").to_string(), st(color))]),
                None => Line::from(Span::styled(if s.exact_cost.is_some() { "exact (reported)" } else { "estimate (list prices)" }, st(th.faint))),
            },
            match balance_str(s) {
                Some(b) => Line::from(vec![Span::styled("SubRouter", st(th.accent2).add_modifier(Modifier::BOLD)), Span::styled(b.trim_start_matches("SubRouter").to_string(), st(th.dim))]),
                None => Line::default(),
            },
        ]),
        a,
    );

    // tools
    let b = panel("tools", th);
    let a = b.inner(p_tools).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_tools);
    // the name column as wide as the longest name ("ScheduleWakeup" ran into its count), cut only when the panel is
    // too narrow for the counts beside it
    let nw = tools.iter().map(|t| t.0.width()).max().unwrap_or(0).min((a.width as usize).saturating_sub(18)).max(4);
    let mut rows: Vec<Line> = tools
        .iter()
        .map(|(n, c, err, secs)| {
            let name: String = if n.width() > nw { format!("{}…", n.chars().take(nw - 1).collect::<String>()) } else { n.clone() };
            Line::from(vec![
                Span::styled(format!("{name:<nw$} "), st(th.accent2).add_modifier(Modifier::BOLD)),
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

    // files: what this conversation changed (git) as a total and the files that fit, each with its counts; a click
    // here (or /diff) opens all of them with their diffs. Outside git: the files its tools named.
    let b = panel("files", th);
    let a = b.inner(p_files).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_files);
    let rows: Vec<Line> = if !v.changed.is_empty() {
        let (add, del) = v.changed.iter().fold((0, 0), |(x, y), c| (x + c.added, y + c.removed));
        let mut rows = vec![Line::from(vec![
            Span::styled(format!("{} ", files_n(v.changed.len())), st(th.text)),
            Span::styled(format!("+{add} "), st(th.ok)),
            Span::styled(format!("−{del}"), st(th.err)),
            Span::styled("  click: diffs", st(th.faint)),
        ])];
        let room = (a.height as usize).saturating_sub(1);
        let shown = if v.changed.len() > room { room.saturating_sub(1) } else { room };
        for c in v.changed.iter().take(shown) {
            let counts = format!(" +{} −{}", c.added, c.removed);
            let path = fit_path(&super::plain(&c.path), (a.width as usize).saturating_sub(counts.width()));
            rows.push(Line::from(vec![Span::styled(path, st(th.dim)), Span::styled(format!(" +{}", c.added), st(th.ok)), Span::styled(format!(" −{}", c.removed), st(th.err))]));
        }
        if v.changed.len() > shown {
            rows.push(Line::from(Span::styled(format!("+{} more", v.changed.len() - shown), st(th.faint))));
        }
        rows
    } else if files.is_empty() {
        vec![Line::from(Span::styled("no changes in the repo yet", st(th.faint)))]
    } else {
        // nothing changed in the repo (git), but an edit tool wrote files: elsewhere, e.g. scratch files in /tmp
        let mut rows = vec![Line::from(Span::styled(format!("no changes in the repo · {} written outside it", files_n(files.len())), st(th.faint)))];
        rows.extend(files.iter().take((a.height as usize).saturating_sub(1)).map(|p| Line::from(Span::styled(fit_path(&super::plain(p), a.width as usize), st(th.dim)))));
        rows
    };
    f.render_widget(Paragraph::new(rows), a);

    let b = Panel::bordered().border_type(BorderType::Rounded).border_style(st(th.border));
    let hint = composer_rows(v, th, "›", "Message tazuna…", composer.width.saturating_sub(2));
    f.render_widget(Paragraph::new(hint).block(b), composer);

    Areas { pulse: p_tools, last_block, transcript: inner, rows: trows, links: vec![], files: Some(p_files), diff_files: vec![] }
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
        let n = super::plain(short_name(&t.name));
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
