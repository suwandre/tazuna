//! Replays a recorded run (`{"t": ms, "e": event}` per line) on a virtual clock.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::Path;

pub struct Replay {
    pub prompt: String,
    pub effort: String,
    events: Vec<(f64, Value)>,
    next: usize,
    pub clock: f64,
    pub speed: f64,
    pub paused: bool,
}

/// Idle gaps (waiting on the model) longer than this are shortened so the replay stays watchable.
const MAX_GAP_MS: f64 = 1200.0;

impl Replay {
    pub fn load(path: &Path, raw_timing: bool) -> Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let (mut events, mut last, mut shift) = (Vec::new(), 0.0, 0.0);
        let (mut prompt, mut effort) = (String::new(), String::from("?"));
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let r: Value = serde_json::from_str(line)?;
            if let Some(p) = r["prompt"].as_str() {
                // header written by recordings/record.py: the prompt and effort are not part of the stream
                prompt = p.to_string();
                effort = r["effort"].as_str().unwrap_or("?").to_string();
                continue;
            }
            let t = r["t"].as_f64().unwrap_or(last);
            if !raw_timing && t - last > MAX_GAP_MS {
                shift += t - last - MAX_GAP_MS;
            }
            last = t;
            events.push((t - shift, r["e"].clone()));
        }
        Ok(Replay { prompt, effort, events, next: 0, clock: 0.0, speed: 1.0, paused: false })
    }

    pub fn finished(&self) -> bool {
        self.next == self.events.len()
    }

    /// Advance the clock by `dt_ms` real milliseconds and return the events that became due.
    pub fn tick(&mut self, dt_ms: f64) -> &[(f64, Value)] {
        if !self.paused {
            self.clock += dt_ms * self.speed;
        }
        let start = self.next;
        while self.next < self.events.len() && self.events[self.next].0 <= self.clock {
            self.next += 1;
        }
        &self.events[start..self.next]
    }

    /// Jump straight to `n` events in (snapshot mode).
    pub fn take(&mut self, n: usize) -> &[(f64, Value)] {
        let end = n.min(self.events.len());
        let start = self.next.min(end);
        self.next = end;
        self.clock = self.events.get(end.saturating_sub(1)).map_or(0.0, |e| e.0);
        &self.events[start..end]
    }
}
