//! Mouse selection inside the transcript. The app keeps the mouse (wheel scrolling), so the terminal cannot select;
//! dragging over the transcript selects its text only (never the side panel), cmd-c / ctrl-c copies it.
//!
//! A selection holds transcript lines, not screen rows: it stays on its text while the transcript scrolls, a drag past
//! the transcript's top or bottom edge scrolls it (as in any terminal or editor), and a copy takes every selected line,
//! on screen or not. Until 0.1.58 it held screen rows, so a scroll moved the highlight onto other text and a selection
//! could not grow past the window (a user, 2026-11-05).

use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, text::Line, widgets::{Paragraph, Widget}};

pub struct Selection {
    /// Where the drag started and where it is now: (screen column, transcript line counted from its first line).
    pub anchor: (u16, usize),
    pub head: (u16, usize),
    pub area: Rect,
    /// While dragging, the pointer is above (-1) or below (1) the transcript: the main loop scrolls that way.
    pub edge: i8,
}

impl Selection {
    /// A drag starting at (x, y) on screen, `top` the transcript line on the area's first row.
    pub fn start(x: u16, y: u16, area: Rect, top: usize) -> Option<Self> {
        let at = (x, top + y.saturating_sub(area.y) as usize);
        area.contains((x, y).into()).then_some(Selection { anchor: at, head: at, area, edge: 0 })
    }

    /// Follows the pointer; past the top or bottom edge it holds the edge row and asks for a scroll that way.
    pub fn drag(&mut self, x: u16, y: u16, top: usize) {
        let a = self.area;
        self.edge = if y < a.y { -1 } else if y >= a.bottom() { 1 } else { 0 };
        let row = y.clamp(a.y, a.bottom() - 1);
        self.head = (x.clamp(a.x, a.right() - 1), top + (row - a.y) as usize);
    }

    /// After the transcript scrolled (`top` its first line on screen now): a head held at an edge moves with it.
    pub fn follow(&mut self, top: usize) {
        match self.edge {
            -1 => self.head.1 = top,
            1 => self.head.1 = top + self.area.height.saturating_sub(1) as usize,
            _ => {}
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Start and end in reading order.
    fn ends(&self) -> ((u16, usize), (u16, usize)) {
        let (a, b) = (self.anchor, self.head);
        if (a.1, a.0) <= (b.1, b.0) { (a, b) } else { (b, a) }
    }

    /// Columns selected on transcript line `line`, like text selection: the first line from the start, the last up to
    /// the end.
    fn columns(&self, line: usize) -> Option<(u16, u16)> {
        let ((x0, l0), (x1, l1)) = self.ends();
        if line < l0 || line > l1 {
            return None;
        }
        let from = if line == l0 { x0 } else { self.area.x };
        let to = if line == l1 { x1 } else { self.area.right() - 1 };
        Some((from, to))
    }

    /// Marks the selected cells on screen; `top` is the transcript line on the area's first row.
    pub fn highlight(&self, buf: &mut Buffer, top: usize) {
        for y in self.area.y..self.area.bottom() {
            if let Some((from, to)) = self.columns(top + (y - self.area.y) as usize) {
                for x in from..=to {
                    if let Some(c) = buf.cell_mut((x, y)) {
                        c.modifier.insert(Modifier::REVERSED);
                    }
                }
            }
        }
    }

    /// The selected text of the whole transcript (`lines`, as drawn at the area's width), on screen or not: trailing
    /// spaces dropped, the indent all lines share removed.
    pub fn text(&self, lines: &[Line<'static>]) -> String {
        let ((_, l0), (_, l1)) = self.ends();
        if l0 >= lines.len() {
            return String::new();
        }
        let l1 = l1.min(lines.len() - 1);
        // the selected lines drawn off screen exactly as on screen, so a cell is a cell either way (wide characters too)
        let mut buf = Buffer::empty(Rect::new(self.area.x, 0, self.area.width, (l1 - l0 + 1).min(u16::MAX as usize) as u16));
        Paragraph::new(lines[l0..=l1].to_vec()).render(buf.area, &mut buf);
        let rows: Vec<String> = (l0..=l1)
            .filter_map(|line| {
                let (from, to) = self.columns(line)?;
                let y = (line - l0) as u16;
                let row: String = (from..=to).filter_map(|x| buf.cell((x, y)).map(|c| crate::ui::plain(c.symbol()))).collect();
                Some(row.trim_end().to_string())
            })
            .collect();
        // the first row starts where the drag did, so only the full rows tell the transcript's indent
        let full = if rows.len() > 1 { &rows[1..] } else { &rows[..] };
        let indent = full.iter().filter(|r| !r.is_empty()).map(|r| r.len() - r.trim_start().len()).min().unwrap_or(0);
        rows.iter()
            .enumerate()
            .map(|(i, r)| if i == 0 && rows.len() > 1 { r.trim_start() } else { r.get(indent..).unwrap_or("") })
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches('\n')
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_transcript_text_in_reading_order() {
        let area = Rect::new(0, 0, 12, 3);
        let lines: Vec<Line<'static>> = ["  alpha beta", "  gamma", "  delta"].iter().map(|t| Line::from(t.to_string())).collect();
        // dragged upwards from "delta" back into "beta"; the side panel (column 14+) is outside the area
        let mut s = Selection::start(6, 2, area, 0).unwrap();
        s.drag(8, 0, 0);
        assert_eq!(s.text(&lines), "beta\ngamma\ndelta");
        s.drag(30, 0, 0);
        assert_eq!(s.head, (11, 0), "the pointer is held inside the transcript");
        assert!(Selection::start(15, 0, area, 0).is_none(), "a drag that starts in the side panel selects nothing");
        let mut b2 = Buffer::empty(Rect::new(0, 0, 20, 3));
        Selection { anchor: (2, 1), head: (6, 1), area, edge: 0 }.highlight(&mut b2, 0);
        assert!(b2[(4, 1)].modifier.contains(Modifier::REVERSED) && !b2[(7, 1)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn a_selection_stays_on_its_text_and_grows_past_the_window() {
        // a transcript of 10 lines in a 3-row area, scrolled so lines 5-7 show
        let area = Rect::new(0, 2, 20, 3);
        let lines: Vec<Line<'static>> = (0..10).map(|i| Line::from(format!("line {i}"))).collect();
        let mut s = Selection::start(0, 3, area, 5).unwrap(); // row 3 = line 6
        assert_eq!(s.anchor, (0, 6));
        // dragged above the transcript: it asks to scroll up and holds the top row
        s.drag(5, 0, 5);
        assert_eq!((s.edge, s.head), (-1, (5, 5)));
        // the loop scrolled two lines up: the head follows to the new top
        s.follow(3);
        assert_eq!(s.head, (5, 3));
        assert_eq!(s.text(&lines), "3\nline 4\nline 5\nl", "line 3 from column 5 to line 6 up to column 0");
        // on screen the highlight sits on the same text after the scroll: line 4 is row 3 now
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 6));
        s.highlight(&mut buf, 3);
        assert!(buf[(0, 3)].modifier.contains(Modifier::REVERSED) && !buf[(0, 0)].modifier.contains(Modifier::REVERSED));
        // released back inside: no more scrolling
        s.drag(5, 3, 3);
        assert_eq!(s.edge, 0);
    }
}
