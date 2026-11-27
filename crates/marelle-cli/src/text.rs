// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Marelle contributors

//! Measuring and cutting text the way a terminal draws it.
//!
//! A terminal paints text into cells. A Latin letter takes one cell, a
//! Chinese, Japanese or Korean character takes two, and a combining mark (a
//! Thai vowel sign, a Devanagari matra, an accent) takes none, because it is
//! drawn on top of the letter before it. Counting `char`s gets all three
//! wrong: frames drift, wrapped lines overflow and the cursor lands on the
//! wrong cell. Everything here counts cells instead, and cuts only between
//! grapheme clusters, so a letter is never parted from its marks.
//!
//! Text stays in logical order. Reordering right-to-left scripts is the
//! terminal's job, because only the terminal knows where the cells are.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Shown instead of a control character, so that content can never inject
/// terminal escape sequences into the interface.
const REPLACEMENT: &str = "\u{FFFD}";

/// Cells a tab is drawn as.
const TAB_CELLS: usize = 4;

/// How many terminal cells `text` occupies.
pub(crate) fn width(text: &str) -> usize {
    text.width()
}

/// Calls `visit` for every cell group of `text`: a grapheme cluster and the
/// cells it takes. Control characters arrive as one replacement cell each.
/// Returns early when `visit` says `false`.
fn each_cell(text: &str, mut visit: impl FnMut(&str, usize) -> bool) {
    for cluster in text.graphemes(true) {
        if cluster.chars().any(char::is_control) {
            for _ in cluster.chars() {
                if !visit(REPLACEMENT, 1) {
                    return;
                }
            }
        } else if !visit(cluster, cluster.width()) {
            return;
        }
    }
}

/// `text` cut to `columns` cells and padded with spaces up to exactly that
/// many. A wide character that would straddle the edge is left out.
pub(crate) fn fit(text: &str, columns: usize) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    each_cell(text, |glyph, cells| {
        if used + cells > columns {
            return false;
        }
        out.push_str(glyph);
        used += cells;
        true
    });
    out.push_str(&" ".repeat(columns - used));
    out
}

/// `text` as it is, or cut to fit `columns` cells and ended with an ellipsis.
pub(crate) fn ellipsize(text: &str, columns: usize) -> String {
    if width(text) <= columns {
        return text.to_owned();
    }
    let room = columns.saturating_sub(1);
    let mut out = String::new();
    let mut used = 0usize;
    each_cell(text, |glyph, cells| {
        if used + cells > room {
            return false;
        }
        out.push_str(glyph);
        used += cells;
        true
    });
    out.push('…');
    out
}

/// How many chars at the start of `line` fit into `columns` cells, counting
/// whole grapheme clusters only.
pub(crate) fn chars_within(line: &str, columns: usize) -> usize {
    let mut used = 0usize;
    let mut chars = 0usize;
    for cluster in line.graphemes(true) {
        let cells = cluster.width();
        if used + cells > columns {
            break;
        }
        used += cells;
        chars += cluster.chars().count();
    }
    chars
}

/// The char index of the grapheme boundary before `cursor`.
pub(crate) fn previous_boundary(text: &str, cursor: usize) -> usize {
    let Some(target) = cursor.checked_sub(1) else {
        return 0;
    };
    let mut start = 0usize;
    let mut last_start = 0usize;
    for cluster in text.graphemes(true) {
        let end = start + cluster.chars().count();
        last_start = start;
        if target < end {
            return start;
        }
        start = end;
    }
    last_start
}

/// The char index of the grapheme boundary after `cursor`.
pub(crate) fn next_boundary(text: &str, cursor: usize) -> usize {
    let mut end = 0usize;
    for cluster in text.graphemes(true) {
        end += cluster.chars().count();
        if end > cursor {
            return end;
        }
    }
    end
}

/// Text laid out into lines of at most a given number of cells, together
/// with where the cursor sits before each char.
///
/// Lines are cut by cells and never inside a grapheme cluster. A line that
/// exactly fills the width is closed at once, so the cursor behind its last
/// cell is on the next line, where the next letter will appear. A newline
/// straight after a full line adds nothing, because the line was already
/// closed.
pub(crate) struct Layout {
    lines: Vec<String>,
    spots: Vec<(usize, usize)>,
}

impl Layout {
    pub(crate) fn new(text: &str, columns: usize) -> Self {
        let mut flow = Flow {
            columns: columns.max(1),
            lines: Vec::new(),
            current: String::new(),
            used: 0,
            wrapped: false,
            newline_after_wrap: false,
            spots: Vec::new(),
        };
        for cluster in text.graphemes(true) {
            if cluster.chars().any(char::is_control) {
                for character in cluster.chars() {
                    match character {
                        '\n' => flow.newline(),
                        '\t' => flow.tab(),
                        _ => flow.glyph(REPLACEMENT, 1, 1),
                    }
                }
            } else {
                flow.glyph(cluster, cluster.width(), cluster.chars().count());
            }
        }
        flow.finish(text.ends_with('\n'))
    }

    /// The lines, each at most `columns` cells wide.
    pub(crate) fn into_lines(self) -> Vec<String> {
        self.lines
    }

    /// The line and cell column of the cursor when it stands before the
    /// `cursor`-th char. A cursor past the end is taken as the end.
    pub(crate) fn spot(&self, cursor: usize) -> (usize, usize) {
        self.spots
            .get(cursor)
            .or_else(|| self.spots.last())
            .copied()
            .unwrap_or((0, 0))
    }
}

struct Flow {
    columns: usize,
    lines: Vec<String>,
    current: String,
    used: usize,
    /// The last line was closed because it was full.
    wrapped: bool,
    /// The newline that came right after such a full line.
    newline_after_wrap: bool,
    spots: Vec<(usize, usize)>,
}

impl Flow {
    fn spot(&self) -> (usize, usize) {
        (self.lines.len(), self.used)
    }

    fn close(&mut self) {
        self.lines.push(std::mem::take(&mut self.current));
        self.used = 0;
    }

    fn put(&mut self, glyph: &str, cells: usize) {
        self.newline_after_wrap = false;
        self.wrapped = false;
        if self.used > 0 && self.used + cells > self.columns {
            self.close();
        }
        self.current.push_str(glyph);
        self.used += cells;
        if self.used >= self.columns {
            self.close();
            self.wrapped = true;
        }
    }

    fn glyph(&mut self, glyph: &str, cells: usize, chars: usize) {
        let spot = self.spot();
        self.spots.extend(std::iter::repeat_n(spot, chars));
        self.put(glyph, cells);
    }

    fn tab(&mut self) {
        self.spots.push(self.spot());
        for _ in 0..TAB_CELLS {
            self.put(" ", 1);
        }
    }

    fn newline(&mut self) {
        self.spots.push(self.spot());
        let was_wrapped = self.wrapped;
        if !was_wrapped || !self.current.is_empty() {
            self.close();
        }
        self.wrapped = false;
        self.newline_after_wrap = was_wrapped;
    }

    fn finish(mut self, ends_with_newline: bool) -> Layout {
        self.spots.push(self.spot());
        if !self.current.is_empty()
            || self.lines.is_empty()
            || (ends_with_newline && !self.newline_after_wrap)
        {
            self.close();
        }
        Layout {
            lines: self.lines,
            spots: self.spots,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_follow_the_script_not_the_char_count() {
        assert_eq!(width("abc"), 3);
        // Ideographs and Hangul take two cells each.
        assert_eq!(width("日本語"), 6);
        assert_eq!(width("한국어"), 6);
        assert_eq!(width("ＡＢ"), 4);
        // A combining accent adds nothing.
        assert_eq!(width("e\u{301}"), 1);
        // Thai: a consonant and its vowel sign are one cell.
        assert_eq!(width("\u{e01}\u{e47}"), 1);
        // Arabic letters are one cell each. A lam followed by an alef is the
        // one exception: the library counts that pair as a single ligature.
        assert_eq!(width("مرحبا"), 5);
    }

    #[test]
    fn fitting_pads_and_cuts_by_cells() {
        assert_eq!(fit("abcdef", 4), "abcd");
        assert_eq!(fit("xy", 4), "xy  ");
        assert_eq!(fit("日本語", 6), "日本語");
        // The third ideograph would straddle the edge, so it is left out.
        assert_eq!(fit("日本語", 5), "日本 ");
        assert_eq!(width(&fit("日本語", 5)), 5);
        assert_eq!(fit("e\u{301}e\u{301}", 1), "e\u{301}");
    }

    #[test]
    fn control_characters_become_replacements() {
        assert_eq!(fit("a\u{1b}b", 3), "a\u{FFFD}b");
        assert_eq!(fit("a\r\nb", 4), "a\u{FFFD}\u{FFFD}b");
    }

    #[test]
    fn ellipsis_ends_a_text_that_does_not_fit() {
        assert_eq!(ellipsize("short", 10), "short");
        assert_eq!(ellipsize("abcdefgh", 5), "abcd…");
        assert_eq!(ellipsize("日本語です", 6), "日本…");
        assert_eq!(width(&ellipsize("日本語です", 6)), 5);
    }

    #[test]
    fn wrapping_counts_cells() {
        assert_eq!(Layout::new("日本語", 4).into_lines(), ["日本", "語"]);
        // A wide character that does not fit moves to the next line whole.
        assert_eq!(Layout::new("a日", 2).into_lines(), ["a", "日"]);
        // Even a line narrower than one character keeps it in one piece.
        assert_eq!(Layout::new("日本", 1).into_lines(), ["日", "本"]);
        // A mark stays with its letter.
        assert_eq!(
            Layout::new("e\u{301}e\u{301}e\u{301}", 2).into_lines(),
            ["e\u{301}e\u{301}", "e\u{301}"]
        );
    }

    #[test]
    fn wrapping_keeps_newlines_and_long_words() {
        let wrap = |text: &str, columns| Layout::new(text, columns).into_lines();
        assert_eq!(
            wrap("alpha beta\nsecond", 5),
            ["alpha", " beta", "secon", "d"]
        );
        assert_eq!(wrap("", 10), [""]);
        assert_eq!(wrap("one\n", 10), ["one", ""]);
        assert_eq!(wrap("abcde\n", 5), ["abcde"]);
        assert_eq!(wrap("abcde\n\n", 5), ["abcde", "", ""]);
        assert_eq!(wrap("a\tb", 80), ["a    b"]);
    }

    #[test]
    fn cursor_spots_follow_the_lines() {
        let spot = |text: &str, cursor, columns| Layout::new(text, columns).spot(cursor);
        assert_eq!(spot("abcd", 4, 4), (1, 0));
        assert_eq!(spot("ab\ncd", 3, 4), (1, 0));
        assert_eq!(spot("abcdef", 5, 4), (1, 1));
        // After two ideographs of a four-cell line the cursor has wrapped.
        assert_eq!(spot("日本語", 2, 4), (1, 0));
        assert_eq!(spot("日本語", 3, 4), (1, 2));
        // A mark does not move the cursor by a cell.
        assert_eq!(spot("e\u{301}x", 2, 10), (0, 1));
        // After the tab's four cells.
        assert_eq!(spot("a\tb", 2, 80), (0, 5));
        // Past the end counts as the end.
        assert_eq!(spot("ab", 99, 10), (0, 2));
    }

    #[test]
    fn a_newline_after_a_full_line_adds_no_blank_line_for_the_cursor() {
        let layout = Layout::new("abcd\n", 4);
        assert_eq!(layout.spot(4), (1, 0));
        assert_eq!(layout.spot(5), (1, 0));
        assert_eq!(layout.into_lines(), ["abcd"]);
    }

    #[test]
    fn boundaries_skip_whole_clusters() {
        let text = "ae\u{301}x";
        assert_eq!(next_boundary(text, 0), 1);
        assert_eq!(next_boundary(text, 1), 3);
        assert_eq!(next_boundary(text, 3), 4);
        assert_eq!(next_boundary(text, 4), 4);
        assert_eq!(previous_boundary(text, 4), 3);
        assert_eq!(previous_boundary(text, 3), 1);
        assert_eq!(previous_boundary(text, 1), 0);
        assert_eq!(previous_boundary(text, 0), 0);
        // A carriage return and a line feed are one cluster.
        assert_eq!(next_boundary("a\r\nb", 1), 3);
    }

    #[test]
    fn columns_map_back_to_chars_in_whole_clusters() {
        assert_eq!(chars_within("日本語", 4), 2);
        assert_eq!(chars_within("日本語", 3), 1);
        assert_eq!(chars_within("abc", 10), 3);
        assert_eq!(chars_within("e\u{301}e\u{301}", 1), 2);
    }
}
