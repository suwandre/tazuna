//! Cockpit: transcript plus a live instrument panel (context, cache, tokens per turn, cost, tools, files).

use super::*;
use crate::session::Block;
use ratatui::{
    layout::{Constraint, Layout, Margin},
    widgets::{Block as Panel, BorderType, LineGauge, Sparkline, SparklineBar, Wrap},
};

pub(super) const OPTS: Opts = Opts { group_tools: false, durations: true };
const SIDEBAR: u16 = 38;
const READING_WIDTH: u16 = 88;

fn panel<'a>(title: &'a str, th: &Theme) -> Panel<'a> {
    hot_panel(title, th, false)
}

/// A panel that opens something on a click: under the mouse its border and title take the accent colour, so it reads
/// as clickable (a user saw no change over "tools" or "tokens", 2026-11-05).
fn hot_panel<'a>(title: &'a str, th: &Theme, hot: bool) -> Panel<'a> {
    let (border, title_color) = if hot { (th.accent, th.accent) } else { (th.border, th.dim) };
    Panel::bordered()
        .border_type(BorderType::Rounded)
        .border_style(st(border))
        .title(Line::from(vec![Span::raw(" "), Span::styled(title, st(title_color).add_modifier(Modifier::BOLD)), Span::raw(" ")]))
}

/// The mouse is over `r`.
fn under_mouse(v: &View, r: Rect) -> bool {
    v.mouse.is_some_and(|(x, y)| r.contains(ratatui::layout::Position::new(x, y)))
}

pub fn draw(f: &mut Frame, s: &Session, v: &View, th: &Theme) -> Areas {
    let ch = if v.live { composer_height(v, f.area().width) } else { 2 };
    let [header, metadata, body, composer] = Layout::vertical([Constraint::Length(1), Constraint::Length(u16::from(!v.live)), Constraint::Min(3), Constraint::Length(ch)]).areas(f.area());
    if !v.live { draw_replay_metadata(f, metadata, v, th); }
    let (glyph, label, color) = activity(s, v, th);

    let left = Line::from(vec![
        Span::styled(" 手 ", Style::new().fg(th.accent).bg(th.field).add_modifier(Modifier::BOLD)),
        Span::styled("tazuna ", Style::new().fg(th.text).bg(th.field).add_modifier(Modifier::BOLD)),
    ]);
    // a terminal under 110 columns (80x24, macOS Terminal's own size) has no room for the instrument column: the
    // transcript gets the width and the header the cost (until 0.1.180 the column took half of 80 and answers wrapped at 38)
    let narrow = body.width < 110;
    // full never asks: its pill in the header too, which every width and state shows (the box's hides while typing)
    let full = if v.live && v.mode == "full" { [mode_pill("full", th, false), vec![Span::raw("   ")]].concat() } else { vec![] };
    let status_at = full.len();
    let mut right = [full, vec![
        Span::styled(format!("{glyph} {label}"), st(color)),
        Span::styled("   ", st(th.faint)),
        Span::styled(if !v.live && s.model.is_empty() { "recorded session".into() } else { model_short(&s.model) }, st(th.text)),
        Span::styled(format!("  {} ", s.effort), st(th.dim)),
    ]].concat();
    // after the status (behind the full pill where it shows); before it, it read "≈$0● working" from 0.1.209 to 0.1.212
    if narrow {
        right.splice(status_at + 1..status_at + 1, [Span::styled("   ", st(th.faint)), Span::styled(cost_str(s), st(th.ink2))]);
    }
    split_row(f, header, left, Line::from(right), Some(th.surface));

    let [main, side] = Layout::horizontal([Constraint::Min(40), Constraint::Length(if narrow { 0 } else { SIDEBAR })]).areas(body);
    let frame = panel("transcript", th);
    let mut inner = frame.inner(main).inner(Margin { horizontal: 1, vertical: 0 });
    // Direction A: keep the full transcript frame and instruments, but give wide prose a bounded reading measure.
    // Use this same rect for wrapping, selection and hit targets. The rail-hidden narrow layout keeps all its room.
    if !narrow && inner.width > READING_WIDTH {
        inner.x += (inner.width - READING_WIDTH) / 2;
        inner.width = READING_WIDTH;
    }
    f.render_widget(frame, main);
    let (last_block, trows, top_line) = render_transcript(f, inner, transcript(s, v, th, &OPTS, inner.width as usize), v.scroll, th);
    if narrow {
        if !v.live {
            draw_replay_footer(f, composer, v, th);
            return Areas { pulse: header, last_block, transcript: inner, rows: trows, top_line, ..Default::default() };
        }
        let mut b = Panel::bordered().border_type(BorderType::Rounded).border_style(st(if v.asking.is_some() { th.warn } else { th.border }));
        // the mode under a draft too: under 110 columns there is no session panel to say it, and the placeholder's pill goes
        // while typing (design review 13)
        if !v.input.is_empty() || v.asking.is_some() {
            b = b.title_bottom(Line::from([mode_pill(&v.mode, th, v.mode == "full"), vec![Span::raw(" ")]].concat()).right_aligned());
        }
        let hint = composer_rows(v, th, "›", "Message tazuna…", composer.width.saturating_sub(2));
        f.render_widget(Paragraph::new(hint).block(b), composer);
        return Areas { pulse: header, last_block, transcript: inner, rows: trows, top_line, composer: Some(composer.inner(Margin { horizontal: 1, vertical: 1 })), ..Default::default() };
    }

    let tools = tool_tally(s);
    let tools_height = tools.len().max(1) as u16 + if s.turn_usage.is_empty() { 3 } else { 2 };
    let accounts = account_rows(s, th);
    let cache_life = s.cache_left(v.clock_ms).zip(s.cache_ttl_ms);
    // Reclaim genuinely empty rows before the layout compresses the chart and pace summary. In a live rail with
    // several tool types, reserving absent cache-life and second-account rows clipped both the zero axis and TTFT.
    // Keep the usual spacing when there is room, and never remove a populated cache/account row.
    let crowded = !s.turn_usage.is_empty() && side.height < 5 + 6 + 9 + 5 + tools_height + 3;
    let context_rows = if crowded && cache_life.is_none() { 3 } else { 4 };
    let cost_rows = if crowded && accounts.1.width() == 0 { 2 } else { 3 };
    let [p_session, p_context, p_tokens, p_cost, p_tools, p_files] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Length(context_rows + 2),
        // the tokens chart once there are calls: before the first, one line (an empty chart with a lone "0" read as broken)
        Constraint::Length(if s.turn_usage.is_empty() { 3 } else { 9 }),
        Constraint::Length(cost_rows + 2),
        // Give the pace summary its extra row instead of leaving a blank row below the tools. The chart keeps its
        // height (and zero axis label) even when the rail has to compress at 120×34.
        Constraint::Length(tools_height),
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
            // model calls, as the row under an answer counts them ("turn 5" after one message, design review 11)
            Line::from(vec![kv("calls", &s.turns.to_string(), th), Span::raw("  "),
                if v.live { Span::styled(format!("live  {}  {}", if s.profile.is_empty() { "lean" } else { &s.profile }, s.mode), st(th.text)) } else { kv("replay", &play, th) }]),
        ]),
        a,
    );

    // context + cache gauges
    let b = panel("context", th);
    let a = b.inner(p_context).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_context);
    let [g1, g2, g3, g4] = Layout::vertical([Constraint::Length(1), Constraint::Length(1), Constraint::Length(1), Constraint::Length(context_rows - 3)]).areas(a);
    // the window as the model reports it: none until claude's first result, none on the own loop, and then the count alone,
    // no gauge (the window stood at "1% of 200k" for a local model, review 12)
    match s.context_window {
        Some(w) => gauge(f, g1, s.context as f64 / w as f64, &format!("{:<12}{} / {}", "context", human(s.context), human(w)), th.accent, th),
        None => f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!("{:<12}", "context"), st(th.text)), Span::styled(human(s.context), st(th.text))])), g1),
    }
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
    gauge(f, g2, s.usage.cache_hit(), &format!("{:<12}{} {:.0}%{last}", "cache read", human(s.usage.cache_read), s.usage.cache_hit() * 100.0), if misses == 0 { th.accent } else { th.warn }, th);
    f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!("{:<12}", "cache write"), st(th.text)), Span::styled(human(s.usage.cache_write), st(th.text)), Span::styled(miss, st(th.warn))])), g3);
    // how long the cache stays warm: after that the next call writes the whole context again
    if let Some((left, ttl)) = cache_life {
        let life = if ttl >= 3_600_000.0 { "1h" } else { "5m" };
        // expired: what the next message costs, before it is sent (Session::rebuild)
        let cold = s.rebuild().map_or(String::new(), |(t, _)| format!(" · next re-reads {}", human(t)));
        let (text, color) = if left > 0.0 { (format!("{} of {life}", clock_str(left)), th.dim) } else { (format!("expired ({life}){cold}"), th.warn) };
        f.render_widget(Paragraph::new(Line::from(vec![Span::styled(format!("{:<12}", "cache life"), st(th.text)), Span::styled(text, st(color))])), g4);
    }

    // tokens per turn
    let mut b = hot_panel("tokens", th, under_mouse(v, p_tokens) && !s.turn_usage.is_empty());
    let a = b.inner(p_tokens).inner(Margin { horizontal: 1, vertical: 0 });
    let mut bars_at = None;
    if s.turn_usage.is_empty() {
        f.render_widget(Paragraph::new(Span::styled("no model calls yet", st(th.faint))), a);
    } else {
        // The fixed-width rail needs two rows for writing rate, end-to-end rate and first-token time. More terminal
        // columns widen the transcript, not this rail, so a single row clipped the last label even in a wide window.
        // An extra composer row can leave only five content rows. Put the caption on the existing lower border
        // then, rather than collapsing the chart's zero axis or hiding a pace label.
        let compact_chart = a.height < 6;
        let [t1, chart, caption, pace] = Layout::vertical([Constraint::Length(1), Constraint::Min(1), Constraint::Length(u16::from(!compact_chart)), Constraint::Length(2)]).areas(a);
        // a y axis beside the bars: the tallest bar's value on top, 0 at the bottom (a user could not read the bare bars)
        let [axis, spark] = Layout::horizontal([Constraint::Length(5), Constraint::Min(1)]).areas(chart);
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
        let data: Vec<u64> = s.turn_usage.iter().map(|u| u.output).collect();
        let first = data.len().saturating_sub(spark.width as usize);
        let top = data[first..].iter().copied().max().unwrap_or(0);
        // one bar per model call, the newest on the right; the mouse over a bar names that call, a click lists them all
        let mut label = match v.hover_call.filter(|i| *i < data.len()) {
            Some(i) => {
                let c = s.calls.get(i);
                let mut l = vec![Span::styled(format!("#{} ", i + 1), st(th.faint)), Span::styled(human(data[i]), st(th.text)), Span::styled(" out", st(th.faint))];
                if let Some(ms) = c.and_then(|c| c.ttft_ms) {
                    l.extend([Span::styled(" · ", st(th.faint)), Span::styled(format!("{:.1}s", ms / 1000.0), st(th.text)), Span::styled(" first", st(th.faint))]);
                }
                if let Some(c) = c.filter(|c| c.gen_ms > 0.0) {
                    l.extend([Span::styled(" · ", st(th.faint)), Span::styled(format!("{:.0}", c.out as f64 / c.gen_ms * 1000.0), st(th.text)), Span::styled(" tok/s", st(th.faint))]);
                }
                Line::from(l)
            }
            None if data.is_empty() => Line::from(Span::styled("output per model call", st(th.faint))),
            None => Line::from(Span::styled(format!("out per call · #{}–{} · click", first + 1, data.len()), st(th.faint))),
        };
        if compact_chart {
            label.spans.insert(0, Span::raw(" "));
            label.spans.push(Span::raw(" "));
            b = b.title_bottom(label);
        } else {
            f.render_widget(Paragraph::new(label), caption);
        }
        let mut ylab = vec![Line::from(Span::styled(format!("{:>4}", human(top)), st(th.faint)))];
        ylab.extend((1..axis.height.saturating_sub(1)).map(|_| Line::default()));
        if axis.height > 1 {
            ylab.push(Line::from(Span::styled(format!("{:>4}", "0"), st(th.faint))));
        }
        f.render_widget(Paragraph::new(ylab), axis);
        // the call in flight, live: the wait for its first token, then how fast it writes (≈: its output is estimated until
        // it ends); between calls the last one's exact figures
        let secs = |ms: f64| format!("{:.1}s", ms / 1000.0);
        let pace_line = match (s.in_flight(), s.calls.last()) {
            (Some((start, None, _)), _) => vec![Span::styled("waiting for first token · ", st(th.faint)), Span::styled(secs(v.clock_ms - start), st(th.text))],
            (Some((start, Some(first), est)), _) => {
                let rate = if v.clock_ms - first > 500.0 { format!("≈{:.0}", est / (v.clock_ms - first) * 1000.0) } else { "…".into() };
                vec![Span::styled(rate, st(th.text)), Span::styled(" tok/s · ", st(th.faint)), Span::styled(secs(first - start), st(th.text)), Span::styled(" to first token", st(th.faint))]
            }
            // between calls the session's figures, as the run's summary line has them: the last call's alone (743 tok/s beside
            // the summary's 788, a user asking which was right, 2026-11-12) are one click away, on the chart above
            (None, Some(_)) => {
                let (ttft, tps, e2e) = crate::session::speed(&s.calls);
                let mut l = vec![];
                if let Some(r) = tps {
                    l.extend([Span::styled(format!("{r:.0}"), st(th.text)), Span::styled(" tok/s writing", st(th.faint))]);
                }
                if let Some(e) = e2e {
                    l.extend([Span::styled(if l.is_empty() { "" } else { " · " }, st(th.faint)), Span::styled(format!("{e:.0}"), st(th.text)),
                        Span::styled(if tps.is_some() { " end to end" } else { " tok/s end to end" }, st(th.faint))]);
                }
                if let Some(t) = ttft {
                    l.extend([Span::styled(if l.is_empty() { "" } else { " · " }, st(th.faint)), Span::styled(secs(t), st(th.text)), Span::styled(" to first token", st(th.faint))]);
                }
                l
            }
            (None, None) => vec![],
        };
        f.render_widget(Paragraph::new(Line::from(pace_line)).wrap(Wrap { trim: true }), pace);
        let bars = data[first..].iter().enumerate().map(|(k, &n)| {
            let b = SparklineBar::from(n);
            if v.hover_call == Some(first + k) { b.style(Some(st(th.text))) } else { b }
        });
        f.render_widget(Sparkline::default().data(bars).max(top.max(1)).style(st(th.dim)), spark);
        bars_at = Some((spark, first));
    }
    f.render_widget(b, p_tokens);

    // cost
    let b = hot_panel("cost · click: all accounts", th, under_mouse(v, p_cost));
    let a = b.inner(p_cost).inner(Margin { horizontal: 1, vertical: 0 });
    f.render_widget(b, p_cost);
    let per_turn = if s.turns > 0 { s.cost() / s.turns as f64 } else { 0.0 };
    f.render_widget(
        Paragraph::new(vec![
            Line::from(vec![Span::styled(cost_str(s), st(th.text).add_modifier(Modifier::BOLD)), Span::styled(format!("   {}/call", usd(per_turn)), st(th.dim))]),
            // the account in use, then another only when it needs you, each on a line of its own (on one line the sidebar
            // cut a balance to "$8", 2026-11-02); every account a click away (/usage)
            accounts.0,
            accounts.1,
        ]),
        a,
    );

    // tools
    let a = Panel::bordered().inner(p_tools).inner(Margin { horizontal: 1, vertical: 0 });
    let row_at = |i: usize| Rect::new(a.x, a.y + i as u16, a.width, 1);
    let hot_row = (0..tools.len().min(a.height as usize)).find(|i| under_mouse(v, row_at(*i)));
    let b = hot_panel("tools · click: calls", th, hot_row.is_some());
    f.render_widget(b, p_tools);
    // the name column as wide as the longest name ("ScheduleWakeup" ran into its count), cut only when the panel is
    // too narrow for the counts beside it
    let nw = tools.iter().map(|t| t.0.width()).max().unwrap_or(0).min((a.width as usize).saturating_sub(23)).max(4);
    let mut rows: Vec<Line> = tools
        .iter()
        .enumerate()
        .map(|(i, (n, c, err, secs, open))| {
            let name: String = if n.width() > nw { format!("{}…", n.chars().take(nw - 1).collect::<String>()) } else { n.clone() };
            Line::from(vec![
                Span::styled(format!("{name:<nw$} "), st(th.ink2).add_modifier(Modifier::BOLD)),
                Span::styled(format!("×{c:<3}"), st(th.text)),
                // ticked once every call has run: a call still running, or waiting for a yes, showed "✓" already
                match (*err, *open) {
                    (1.., _) => Span::styled(format!(" ✗{err:<2}"), st(th.err)),
                    (0, 1..) => Span::styled(" …  ", st(th.faint)),
                    (0, 0) => Span::styled(" ✓  ", st(th.ok)),
                },
                // the total and the average a call (a user asked for both: 14 Agent calls at 1842 s said little alone)
                Span::styled(format!("{:>6}", dur(*secs)), st(th.faint)),
                Span::styled(format!(" avg {}", dur(secs / *c.max(&1) as f64)), st(th.dim)),
            ])
            .style(if hot_row == Some(i) { Style::new().bg(th.surface) } else { Style::new() })
        })
        .collect();
    let tool_rows = tools.iter().take(a.height as usize).enumerate().map(|(i, t)| (row_at(i), t.0.clone())).collect();
    if rows.is_empty() {
        rows.push(Line::from(Span::styled("none yet", st(th.faint))));
    }
    f.render_widget(Paragraph::new(rows), a);

    // files: what this conversation changed (git) as a total and the files that fit, each with its counts; a click
    // here (or /diff) opens all of them with their diffs. Outside git: the files its tools named.
    let b = hot_panel("files", th, under_mouse(v, p_files) && !v.changed.is_empty());
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
    } else {
        vec![Line::from(Span::styled("no file changed yet", st(th.faint)))]
    };
    f.render_widget(Paragraph::new(rows), a);

    if v.live {
        let b = Panel::bordered().border_type(BorderType::Rounded).border_style(st(if v.asking.is_some() { th.warn } else { th.border }));
        let hint = composer_rows(v, th, "›", "Message tazuna…", composer.width.saturating_sub(2));
        f.render_widget(Paragraph::new(hint).block(b), composer);
    } else {
        draw_replay_footer(f, composer, v, th);
    }

    Areas { pulse: p_tools, last_block, transcript: inner, rows: trows, files: Some(p_files), cost: Some(p_cost), tool_rows, tokens: Some(p_tokens), bars: bars_at, top_line, composer: v.live.then_some(composer.inner(Margin { horizontal: 1, vertical: 1 })), ..Default::default() }
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

/// (tool, calls, failures, total seconds, calls without a result yet), in first-use order.
fn tool_tally(s: &Session) -> Vec<(String, usize, usize, f64, usize)> {
    let mut out: Vec<(String, usize, usize, f64, usize)> = Vec::new();
    for b in &s.blocks {
        let Block::Tool(t) = b else { continue };
        let n = super::plain(short_name(&t.name));
        let i = match out.iter().position(|r| r.0 == n) {
            Some(i) => i,
            None => {
                out.push((n, 0, 0, 0.0, 0));
                out.len() - 1
            }
        };
        out[i].1 += 1;
        // a call the user said no to is no failure
        out[i].2 += (t.error && !t.declined()) as usize;
        out[i].3 += t.secs().unwrap_or(0.0);
        out[i].4 += t.result.is_none() as usize;
    }
    out
}

/// A duration in its own unit: "7.1s", "30.7m", "1.2h".
fn dur(secs: f64) -> String {
    if secs < 60.0 { format!("{secs:.1}s") } else if secs < 3600.0 { format!("{:.1}m", secs / 60.0) } else { format!("{:.1}h", secs / 3600.0) }
}

/// Shortens a path from the left so the file name stays visible.
fn fit_path(p: &str, width: usize) -> String {
    let n = p.chars().count();
    if n <= width { p.to_string() } else { format!("…{}", p.chars().skip(n + 1 - width).collect::<String>()) }
}
