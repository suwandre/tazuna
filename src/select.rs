//! Mouse selection inside the transcript. The app keeps the mouse (wheel scrolling), so the terminal cannot select;
//! dragging over the transcript selects its text only (never the side panel) and copies it on release.

use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

pub struct Selection {
    /// Where the drag started and where it is now (column, row), both inside `area`.
    pub anchor: (u16, u16),
    pub head: (u16, u16),
    pub area: Rect,
}

impl Selection {
    pub fn start(x: u16, y: u16, area: Rect) -> Option<Self> {
        area.contains((x, y).into()).then_some(Selection { anchor: (x, y), head: (x, y), area })
    }

    /// Follows the pointer, held inside the transcript.
    pub fn drag(&mut self, x: u16, y: u16) {
        let a = self.area;
        self.head = (x.clamp(a.x, a.right() - 1), y.clamp(a.y, a.bottom() - 1));
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Start and end in reading order.
    fn ends(&self) -> ((u16, u16), (u16, u16)) {
        let (a, b) = (self.anchor, self.head);
        if (a.1, a.0) <= (b.1, b.0) { (a, b) } else { (b, a) }
    }

    /// Columns selected on row `y`, like text selection: the first row from the start, the last up to the end.
    fn columns(&self, y: u16) -> Option<(u16, u16)> {
        let ((x0, y0), (x1, y1)) = self.ends();
        if y < y0 || y > y1 {
            return None;
        }
        let from = if y == y0 { x0 } else { self.area.x };
        let to = if y == y1 { x1 } else { self.area.right() - 1 };
        Some((from, to))
    }

    pub fn highlight(&self, buf: &mut Buffer) {
        for y in self.area.y..self.area.bottom() {
            if let Some((from, to)) = self.columns(y) {
                for x in from..=to {
                    if let Some(c) = buf.cell_mut((x, y)) {
                        c.modifier.insert(Modifier::REVERSED);
                    }
                }
            }
        }
    }

    /// The selected text as it reads on screen: trailing spaces dropped, the indent all rows share removed.
    pub fn text(&self, buf: &Buffer) -> String {
        let rows: Vec<String> = (self.area.y..self.area.bottom())
            .filter_map(|y| {
                let (from, to) = self.columns(y)?;
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
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        for (y, t) in ["  alpha beta  side", "  gamma       side", "  delta       side"].iter().enumerate() {
            buf.set_string(0, y as u16, t, ratatui::style::Style::new());
        }
        // dragged upwards from "delta" back into "beta"; the side panel (column 14+) is outside the area
        let mut s = Selection::start(6, 2, area).unwrap();
        s.drag(8, 0);
        assert_eq!(s.text(&buf), "beta\ngamma\ndelta");
        s.drag(30, 0);
        assert_eq!(s.head, (11, 0), "the pointer is held inside the transcript");
        assert!(Selection::start(15, 0, area).is_none(), "a drag that starts in the side panel selects nothing");
        let mut b2 = buf.clone();
        Selection { anchor: (2, 1), head: (6, 1), area }.highlight(&mut b2);
        assert!(b2[(4, 1)].modifier.contains(Modifier::REVERSED) && !b2[(7, 1)].modifier.contains(Modifier::REVERSED));
    }
}
