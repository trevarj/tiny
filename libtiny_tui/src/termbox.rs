//! Some utilities for termbox

use crate::config::Style;
use termbox_simple::Termbox;

pub(crate) fn print_chars<T, C>(tb: &mut T, mut pos_x: i32, pos_y: i32, style: Style, chars: C)
where
    C: Iterator<Item = char>,
    T: Termbox,
{
    for char in chars {
        tb.change_cell(pos_x, pos_y, char, style.fg, style.bg);
        pos_x += 1;
    }
}
