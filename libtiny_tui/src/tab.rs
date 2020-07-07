pub use libtiny_ui::TabStyle;
use termbox_simple::{Termbox, TB_UNDERLINE};

use crate::{
    line_split::LineDataCache,
    config::{Colors, Style},
    messaging::MessagingUI,
    notifier::Notifier,
    MsgSource,
};

pub(crate) struct Tab {
    pub(crate) alias: Option<String>,
    pub(crate) widget: MessagingUI,
    pub(crate) src: MsgSource,
    pub(crate) style: TabStyle,
    /// Alt-character to use to switch to this tab.
    pub(crate) switch: Option<char>,
    pub(crate) notifier: Notifier,
}

fn tab_style(style: TabStyle, colors: &Colors) -> Style {
    match style {
        TabStyle::Normal => colors.tab_normal,
        TabStyle::NewMsg => colors.tab_new_msg,
        TabStyle::Highlight => colors.tab_highlight,
    }
}

impl Tab {
    pub(crate) fn visible_name(&self) -> &str {
        match self.alias {
            Some(ref alias) => &alias,
            None => self.src.visible_name(),
        }
    }

    pub(crate) fn set_style(&mut self, style: TabStyle) {
        self.style = style;
    }

    pub(crate) fn update_source<F>(&mut self, f: &F)
    where
        F: Fn(&mut MsgSource),
    {
        f(&mut self.src)
    }

    pub(crate) fn width(&self) -> i32 {
        // TODO: assuming ASCII string here. We should probably switch to a AsciiStr type.
        self.visible_name().len() as i32
    }

    pub(crate) fn draw(
        &self,
        tb: &mut Termbox,
        colors: &Colors,
        mut pos_x: i32,
        pos_y: i32,
        active: bool,
    ) {
        let style: Style = if active {
            colors.tab_active
        } else {
            tab_style(self.style, colors)
        };

        let mut switch_drawn = false;
        for ch in self.visible_name().chars() {
            if Some(ch) == self.switch && !switch_drawn {
                tb.change_cell(pos_x, pos_y, ch, style.fg | TB_UNDERLINE, style.bg);
                switch_drawn = true;
            } else {
                tb.change_cell(pos_x, pos_y, ch, style.fg, style.bg);
            }
            pos_x += 1;
        }
    }
}

const UPDOWN_ARROW: char = '↕'; // 
const UP_ARROW: char = '⬆'; // ▲
const DOWN_ARROW: char = '⬇'; // ▼

pub struct TabWidget {
    /// List of tabs
    pub tabs: Vec<Tab>,
    /// Index of active tab
    active_tab_idx: usize,
    /// Widget width
    width: i32,
    /// Widget height, invalidated on resize and tab addition
    height: Option<i32>,
    /// Maximum height widget can grow to
    max_height: i32,
    /// Line data for calculating line wrapping
    line_data: LineDataCache,
    /// Active line that is visibile in scroll mode
    active_line: Option<i32>,
    /// Is the widget visible?
    visible: bool,
}

impl TabWidget {
    pub fn new(width: i32, max_height: i32) -> TabWidget {
        TabWidget {
            tabs: Vec::new(),
            active_tab_idx: 0,
            width,
            height: Some(1),
            max_height,
            line_data: LineDataCache::new(false),
            active_line: None,
            visible: true,
        }
    }

    pub fn get_active_idx(&self) -> usize {
        self.active_tab_idx
    }

    pub fn resize(&mut self, width: i32) {
        
    }

    pub fn handle_close(&mut self) {
        // invalidate height and scroll_window
    }

    pub fn get_height(&mut self, width: i32) -> i32 {
        let height = 
        // Get cached value or calculate
        if let Some(height) = self.height {
            height
        } else {
            // calculate height
            self.calculate_height(width)
        };

        // Check for scroll fallback
        if height >= self.max_lines || width <= SCROLL_FALLBACK_WIDTH {
            self.scroll_on();
            1
        } else {
            self.scroll_off();
            height
        }
    }

    pub fn draw(&self, tb: &mut Termbox, width: i32, colors: &Colors) {
        // get height
        self.get_height(width);
        // Check if we should scroll
        if self.should_scroll() {
            self.draw_scroll(tb, colors)
        } else {
            self.draw_wrapping(tb, colors)
        }
    }

    fn calculate_height(&mut self, width: i32) -> i32 {
        let tab_name_chars = self.tabs.iter().map(|t| t.visible_name()).collect::<Vec<&str>>().join(" ");
        self.line_data.calculate_height(tab_name_chars.chars(), 0);
        // set scroll_window based on current line
        for (line, split) in self.line_data.get_splits().iter().copied().enumerate().rev() {
            if self.active_tab_idx as i32 >= split {
                // found active line, set it
                // +1 because enumeration starts at 0
                self.active_line = Some(line as i32 + 1);
            }
        }
        self.line_data.get_line_count().unwrap() as i32
    }

    fn draw_scroll(&self, tb: &mut Termbox, colors: &Colors) {
        
    }

    fn draw_wrapping(&self, tb: &mut Termbox, colors: &Colors) {

    }

    fn should_scroll(&self) -> bool {

    }
}

fn arrow_style(tabs: &[Tab], colors: &Colors) -> Style {
    let tab_style = tabs
        .iter()
        .map(|tab| tab.style)
        .max()
        .unwrap_or(TabStyle::Normal);
    match tab_style {
        TabStyle::Normal => colors.tab_normal,
        TabStyle::NewMsg => colors.tab_new_msg,
        TabStyle::Highlight => colors.tab_highlight,
    }
}
