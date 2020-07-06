#![allow(clippy::cognitive_complexity)]
#![allow(clippy::new_without_default)]
#![allow(clippy::too_many_arguments)]

use std::path::PathBuf;
use std::str;
use std::str::SplitWhitespace;
use time::Tm;

use crate::config::{parse_config, Colors, Config};
use crate::editor;
use crate::messaging::{MessagingUI, Timestamp};
use crate::notifier::Notifier;
use crate::statusline::{draw_statusline, statusline_visible};
use crate::tab::{Tab, TabStyle, TabWidget};
use crate::widget::WidgetRet;
use crate::{MsgSource, MsgTarget};

use term_input::{Arrow, Event, Key};
use termbox_simple::Termbox;

#[derive(Debug)]
pub(crate) enum TUIRet {
    Abort,
    KeyHandled,
    KeyIgnored(Key),
    EventIgnored(Event),

    /// INVARIANT: The vec will have at least one char.
    // Can't make MsgSource a ref because of this weird error:
    // https://users.rust-lang.org/t/borrow-checker-bug/5165
    Input {
        msg: Vec<char>,
        from: MsgSource,
    },
}

pub(crate) struct TUI {
    /// Termbox instance
    tb: Termbox,

    /// Color scheme
    colors: Colors,

    /// Max number of message lines
    scrollback: usize,

    tab_widget: TabWidget,
    width: i32,
    height: i32,

    /// Do we want to show statusline?
    show_statusline: bool,
    /// Is there room for statusline?
    statusline_visible: bool,
    /// Config file path
    config_path: Option<PathBuf>,
}

impl TUI {
    pub(crate) fn new(config_path: PathBuf) -> TUI {
        let tb = Termbox::init().unwrap(); // TODO: check errors
        TUI::new_tb(Some(config_path), tb)
    }

    #[cfg(test)]
    pub(crate) fn new_test(w: u16, h: u16) -> TUI {
        let tb = Termbox::init_test(w, h);
        TUI::new_tb(None, tb)
    }

    #[cfg(test)]
    pub(crate) fn get_tb(&self) -> &Termbox {
        &self.tb
    }

    pub(crate) fn activate(&mut self) {
        self.tb.activate()
    }

    fn new_tb(config_path: Option<PathBuf>, tb: Termbox) -> TUI {
        // This is now done in reload_config() below
        // tb.set_clear_attributes(colors.clear.fg as u8, colors.clear.bg as u8);

        let width = tb.width() as i32;
        let height = tb.height() as i32;

        let mut tui = TUI {
            tb,
            colors: Colors::default(),
            scrollback: usize::MAX,
            // TODO: calculate from screen height
            tab_widget: TabWidget::new(width, 4),
            width,
            height,
            show_statusline: false,
            statusline_visible: statusline_visible(width, height),
            config_path,
        };

        // Init "mentions" tab. This needs to happen right after creating the TUI to be able to
        // show any errors in TUI.
        tui.new_server_tab("mentions", None);
        tui.add_client_msg(
            "Any mentions to you will be listed here.",
            &MsgTarget::Server { serv: "mentions" },
        );

        tui.reload_config();
        tui
    }

    fn ignore(&mut self, src: &MsgSource) {
        match src {
            MsgSource::Serv { serv } => {
                self.toggle_ignore(&MsgTarget::AllServTabs { serv: &serv });
            }
            MsgSource::Chan { serv, chan } => {
                self.toggle_ignore(&MsgTarget::Chan {
                    serv: &serv,
                    chan: &chan,
                });
            }
            MsgSource::User { serv, nick } => {
                self.toggle_ignore(&MsgTarget::User {
                    serv: &serv,
                    nick: &nick,
                });
            }
        }
    }

    fn notify(&mut self, words: &mut SplitWhitespace, src: &MsgSource) {
        let words: Vec<&str> = words.collect();

        let mut show_usage = || {
            self.add_client_err_msg(
                "/notify usage: /notify [off|mentions|messages]",
                &MsgTarget::CurrentTab,
            )
        };

        if words.is_empty() {
            self.show_notify_mode(&MsgTarget::CurrentTab);
        } else if words.len() != 1 {
            show_usage();
        } else {
            let notifier = match words[0] {
                "off" => {
                    self.add_client_notify_msg("Notifications turned off", &MsgTarget::CurrentTab);
                    Notifier::Off
                }
                "mentions" => {
                    self.add_client_notify_msg(
                        "Notifications enabled for mentions",
                        &MsgTarget::CurrentTab,
                    );
                    Notifier::Mentions
                }
                "messages" => {
                    self.add_client_notify_msg(
                        "Notifications enabled for all messages",
                        &MsgTarget::CurrentTab,
                    );
                    Notifier::Messages
                }
                _ => {
                    return show_usage();
                }
            };
            // can't use `MsgSource::to_target` here, `Serv` case is different
            let tab_target = match src {
                MsgSource::Serv { ref serv } => MsgTarget::AllServTabs { serv },
                MsgSource::Chan { ref serv, ref chan } => MsgTarget::Chan { serv, chan },
                MsgSource::User { ref serv, ref nick } => MsgTarget::User { serv, nick },
            };
            self.set_notifier(notifier, &tab_target);
        }
    }

    pub(crate) fn try_handle_cmd(&mut self, cmd: &str, src: &MsgSource) -> bool {
        let mut words = cmd.split_whitespace();
        match words.next() {
            Some("clear") => {
                self.clear(&src.to_target());
                true
            }
            Some("ignore") => {
                self.ignore(src);
                true
            }
            Some("notify") => {
                self.notify(&mut words, src);
                true
            }
            Some("switch") => {
                match words.next() {
                    Some(s) => self.switch(s),
                    None => self.add_client_err_msg(
                        "/switch usage: /switch <tab name>",
                        &MsgTarget::CurrentTab,
                    ),
                }
                true
            }
            Some("statusline") => {
                self.toggle_statusline();
                true
            }
            Some("reload") => {
                self.reload_config();
                true
            }
            _ => false,
        }
    }

    pub(crate) fn reload_config(&mut self) {
        if let Some(ref config_path) = self.config_path {
            match parse_config(config_path) {
                Err(err) => {
                    self.add_client_err_msg(
                        &format!("Can't parse TUI config: {:?}", err),
                        &MsgTarget::CurrentTab,
                    );
                }
                Ok(Config { colors, scrollback }) => {
                    self.set_colors(colors);
                    self.scrollback = scrollback.max(1);
                }
            }
        }
    }

    fn set_colors(&mut self, colors: Colors) {
        self.tb
            .set_clear_attributes(colors.clear.fg as u8, colors.clear.bg as u8);
        self.colors = colors;
    }

    fn new_tab(
        &mut self,
        idx: usize,
        src: MsgSource,
        status: bool,
        notifier: Notifier,
        alias: Option<String>,
    ) {
        use std::collections::HashMap;

        let mut switch_keys: HashMap<char, i8> = HashMap::with_capacity(self.tab_widget.tabs.len());
        for tab in &self.tab_widget.tabs {
            if let Some(key) = tab.switch {
                switch_keys.entry(key).and_modify(|e| *e += 1).or_insert(1);
            }
        }

        let switch = {
            let mut ret = None;
            let mut n = 0;
            let visible_name = match alias.as_ref() {
                None => src.visible_name(),
                Some(ref alias) => alias,
            };
            for ch in visible_name.chars() {
                if !ch.is_alphabetic() {
                    continue;
                }
                match switch_keys.get(&ch) {
                    None => {
                        ret = Some(ch);
                        break;
                    }
                    Some(n_) => {
                        if ret == None || n > *n_ {
                            ret = Some(ch);
                            n = *n_;
                        }
                    }
                }
            }
            ret
        };

        let statusline_height = if self.statusline_visible && self.show_statusline {
            1
        } else {
            0
        };
        self.tab_widget.tabs.insert(
            idx,
            Tab {
                alias,
                widget: MessagingUI::new(
                    self.width,
                    self.height - 1 - statusline_height,
                    status,
                    self.scrollback,
                ),
                src,
                style: TabStyle::Normal,
                switch,
                notifier,
            },
        );
    }

    /// Returns index of the new tab if a new tab is created.
    pub(crate) fn new_server_tab(&mut self, serv: &str, alias: Option<String>) -> Option<usize> {
        match self.find_serv_tab_idx(serv) {
            None => {
                let tab_idx = self.tab_widget.tabs.len();
                self.new_tab(
                    tab_idx,
                    MsgSource::Serv {
                        serv: serv.to_owned(),
                    },
                    true,
                    Notifier::Mentions,
                    alias,
                );
                Some(tab_idx)
            }
            Some(_) => None,
        }
    }

    /// Closes a server tab and all associated channel tabs.
    pub(crate) fn close_server_tab(&mut self, serv: &str) {
        if let Some(tab_idx) = self.find_serv_tab_idx(serv) {
            self.tab_widget
                .tabs
                .retain(|tab: &Tab| tab.src.serv_name() != serv);
            if self.tab_widget.get_active_idx() == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
        self.tab_widget.handle_close();
    }

    /// Returns index of the new tab if a new tab is created.
    pub(crate) fn new_chan_tab(&mut self, serv: &str, chan: &str) -> Option<usize> {
        match self.find_chan_tab_idx(serv, chan) {
            None => match self.find_last_serv_tab_idx(serv) {
                None => {
                    self.new_server_tab(serv, None);
                    self.new_chan_tab(serv, chan)
                }
                Some(serv_tab_idx) => {
                    let mut status_val: bool = true;
                    let mut notifier = Notifier::Mentions;
                    for tab in &self.tab_widget.tabs {
                        if let MsgSource::Serv { serv: ref serv_ } = tab.src {
                            if serv == serv_ {
                                status_val = tab.widget.get_ignore_state();
                                notifier = tab.notifier;
                                break;
                            }
                        }
                    }
                    let tab_idx = serv_tab_idx + 1;
                    self.new_tab(
                        tab_idx,
                        MsgSource::Chan {
                            serv: serv.to_owned(),
                            chan: chan.to_owned(),
                        },
                        status_val,
                        notifier,
                        None,
                    );
                    if self.tab_widget.get_active_idx() >= tab_idx {
                        self.next_tab();
                    }
                    if let Some(nick) = self.tab_widget.tabs[serv_tab_idx].widget.get_nick() {
                        self.tab_widget.tabs[tab_idx].widget.set_nick(nick);
                    }
                    Some(tab_idx)
                }
            },
            Some(_) => None,
        }
    }

    pub(crate) fn close_chan_tab(&mut self, serv: &str, chan: &str) {
        if let Some(tab_idx) = self.find_chan_tab_idx(serv, chan) {
            self.tab_widget.tabs.remove(tab_idx);
            if self.tab_widget.get_active_idx() == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }
        self.tab_widget.handle_close();
    }

    /// Returns index of the new tab if a new tab is created.
    pub(crate) fn new_user_tab(&mut self, serv: &str, nick: &str) -> Option<usize> {
        match self.find_user_tab_idx(serv, nick) {
            None => match self.find_last_serv_tab_idx(serv) {
                None => {
                    self.new_server_tab(serv, None);
                    self.new_user_tab(serv, nick)
                }
                Some(tab_idx) => {
                    self.new_tab(
                        tab_idx + 1,
                        MsgSource::User {
                            serv: serv.to_owned(),
                            nick: nick.to_owned(),
                        },
                        true,
                        Notifier::Messages,
                        None,
                    );
                    if let Some(nick) = self.tab_widget.tabs[tab_idx].widget.get_nick() {
                        self.tab_widget.tabs[tab_idx + 1].widget.set_nick(nick);
                    }
                    self.tab_widget.tabs[tab_idx + 1].widget.join(nick, None);
                    Some(tab_idx + 1)
                }
            },
            Some(_) => None,
        }
    }

    pub(crate) fn close_user_tab(&mut self, serv: &str, nick: &str) {
        if let Some(tab_idx) = self.find_user_tab_idx(serv, nick) {
            self.tab_widget.tabs.remove(tab_idx);
            if self.tab_widget.get_active_idx() == tab_idx {
                self.select_tab(if tab_idx == 0 { 0 } else { tab_idx - 1 });
            }
        }

        self.tab_widget.handle_close();
    }

    pub(crate) fn handle_input_event(
        &mut self,
        ev: Event,
        rcv_editor_ret: &mut Option<editor::ResultReceiver>,
    ) -> TUIRet {
        match ev {
            Event::Key(key) => self.keypressed(key, rcv_editor_ret),

            Event::String(str) => {
                // For some reason on my terminal newlines in text are
                // translated to carriage returns when pasting so we check for
                // both just to make sure
                if str.contains('\n') || str.contains('\r') {
                    self.run_editor(&str, rcv_editor_ret);
                } else {
                    // TODO this may be too slow for pasting long single lines
                    for ch in str.chars() {
                        self.handle_input_event(Event::Key(Key::Char(ch)), rcv_editor_ret);
                    }
                }
                TUIRet::KeyHandled
            }

            ev => TUIRet::EventIgnored(ev),
        }
    }

    pub(crate) fn handle_editor_result(
        &mut self,
        editor_ret: editor::Result<Vec<String>>,
    ) -> Option<(Vec<String>, MsgSource)> {
        match editor_ret {
            Err(err) => {
                self.handle_editor_err(err);
                None
            }
            Ok(lines) => {
                let tab = &mut self.tab_widget.tabs[self.tab_widget.get_active_idx()].widget;
                // If there's only one line just add it to the input field, do not send it
                if lines.len() == 1 {
                    tab.set_input_field(&lines[0]);
                    None
                } else {
                    // Otherwise add the lines to text field history and send it
                    for line in &lines {
                        tab.add_input_field_history(line);
                    }
                    Some((
                        lines,
                        self.tab_widget.tabs[self.tab_widget.get_active_idx()]
                            .src
                            .clone(),
                    ))
                }
            }
        }
    }

    /// Edit current input + `str` before sending.
    fn run_editor(&mut self, str: &str, rcv_editor_ret: &mut Option<editor::ResultReceiver>) {
        let tab = &mut self.tab_widget.tabs[self.tab_widget.get_active_idx()].widget;
        let tf = tab.flush_input_field();
        match editor::run(&mut self.tb, &tf, &str, rcv_editor_ret) {
            Ok(()) => {}
            Err(err) => self.handle_editor_err(err),
        }
    }

    fn handle_editor_err(&mut self, err: editor::Error) {
        use std::env::VarError;
        match err {
            editor::Error::Io(err) => {
                self.add_client_err_msg(
                    &format!("Error while running $EDITOR: {:?}", err),
                    &MsgTarget::CurrentTab,
                );
            }
            editor::Error::Var(VarError::NotPresent) => {
                self.add_client_err_msg(
                    "Can't paste multi-line string: \
                             make sure your $EDITOR is set",
                    &MsgTarget::CurrentTab,
                );
            }
            editor::Error::Var(VarError::NotUnicode(_)) => {
                self.add_client_err_msg(
                    "Can't paste multi-line string: \
                             can't parse $EDITOR (not unicode)",
                    &MsgTarget::CurrentTab,
                );
            }
        }
    }

    fn keypressed(
        &mut self,
        key: Key,
        rcv_editor_ret: &mut Option<editor::ResultReceiver>,
    ) -> TUIRet {
        match self.tab_widget.tabs[self.tab_widget.get_active_idx()]
            .widget
            .keypressed(key)
        {
            WidgetRet::KeyHandled => TUIRet::KeyHandled,
            WidgetRet::KeyIgnored => self.handle_keypress(key, rcv_editor_ret),
            WidgetRet::Input(input) => TUIRet::Input {
                msg: input,
                from: self.tab_widget.tabs[self.tab_widget.get_active_idx()]
                    .src
                    .clone(),
            },
            WidgetRet::Remove => unimplemented!(),
            WidgetRet::Abort => TUIRet::Abort,
        }
    }

    fn handle_keypress(
        &mut self,
        key: Key,
        rcv_editor_ret: &mut Option<editor::ResultReceiver>,
    ) -> TUIRet {
        match key {
            Key::Ctrl('n') => {
                self.next_tab();
                TUIRet::KeyHandled
            }

            Key::Ctrl('p') => {
                self.prev_tab();
                TUIRet::KeyHandled
            }

            Key::Ctrl('x') => {
                self.run_editor("", rcv_editor_ret);
                TUIRet::KeyHandled
            }

            Key::AltChar(c) => match c.to_digit(10) {
                Some(i) => {
                    let new_tab_idx: usize = if i as usize > self.tab_widget.tabs.len() || i == 0 {
                        self.tab_widget.tabs.len() - 1
                    } else {
                        i as usize - 1
                    };
                    use std::cmp::Ordering;
                    match new_tab_idx.cmp(&self.tab_widget.get_active_idx()) {
                        Ordering::Greater => {
                            for _ in 0..new_tab_idx - self.tab_widget.get_active_idx() {
                                self.next_tab_();
                            }
                        }
                        Ordering::Less => {
                            for _ in 0..self.tab_widget.get_active_idx() - new_tab_idx {
                                self.prev_tab_();
                            }
                        }
                        Ordering::Equal => {}
                    }
                    self.tab_widget.tabs[self.tab_widget.get_active_idx()]
                        .set_style(TabStyle::Normal);
                    TUIRet::KeyHandled
                }
                None => {
                    // multiple tabs can have same switch character so scan
                    // forwards instead of starting from the first tab
                    for i in 1..=self.tab_widget.tabs.len() {
                        let idx =
                            (self.tab_widget.get_active_idx() + i) % self.tab_widget.tabs.len();
                        if self.tab_widget.tabs[idx].switch == Some(c) {
                            self.select_tab(idx);
                            break;
                        }
                    }
                    TUIRet::KeyHandled
                }
            },

            Key::AltArrow(Arrow::Left) => {
                self.move_tab_left();
                TUIRet::KeyHandled
            }

            Key::AltArrow(Arrow::Right) => {
                self.move_tab_right();
                TUIRet::KeyHandled
            }

            key => TUIRet::KeyIgnored(key),
        }
    }

    /// Handles resize events. Call on SIGWINCH.
    pub(crate) fn resize(&mut self) {
        self.tb.resize();
        self.tb.clear();

        self.width = self.tb.width();
        self.height = self.tb.height();

        self.resize_();
    }

    #[cfg(test)]
    /// Set terminal size. Useful when testing resizing.
    pub(crate) fn set_size(&mut self, w: u16, h: u16) {
        self.tb.set_buffer_size(w, h);

        self.width = i32::from(w);
        self.height = i32::from(h);

        self.resize_();
    }

    fn resize_(&mut self) {
        // self.statusline_visible = statusline_visible(self.width, self.height);
        let statusline_height =
            if statusline_visible(self.width, self.height) && self.show_statusline {
                1
            } else {
                0
            };
        for tab in &mut self.tab_widget.tabs {
            tab.widget
                .resize(self.width, self.height - 1 - statusline_height);
        }
        
        self.tab_widget.resize(self.width);

        // redraw after resize
        self.draw()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Rendering

impl TUI {

    pub(crate) fn draw(&mut self) {
        self.tb.clear();

        let statusline_height = if self.statusline_visible && self.show_statusline {
            1
        } else {
            0
        };

        if self.show_statusline && self.statusline_visible {
            draw_statusline(
                &mut self.tb,
                self.width,
                &self.colors,
                &self.tab_widget.tabs[self.tab_widget.get_active_idx()].visible_name(),
                self.tab_widget.tabs[self.tab_widget.get_active_idx()].notifier,
                self.tab_widget.tabs[self.tab_widget.get_active_idx()]
                    .widget
                    .get_ignore_state(),
            );
        }

        self.tab_widget.tabs[self.tab_widget.get_active_idx()]
            .widget
            .draw(&mut self.tb, &self.colors, 0, statusline_height);

            
        self.tb.present();
    }

    ////////////////////////////////////////////////////////////////////////////
    // Moving between tabs, horizontal scroll updates

    fn select_tab(&mut self, tab_idx: usize) {
        if tab_idx < self.tab_widget.get_active_idx() {
            while tab_idx < self.tab_widget.get_active_idx() {
                self.prev_tab_();
            }
        } else {
            while tab_idx > self.tab_widget.get_active_idx() {
                self.next_tab_();
            }
        }
        self.tab_widget.tabs[self.tab_widget.get_active_idx()].set_style(TabStyle::Normal);
    }

    pub(crate) fn next_tab(&mut self) {
        self.next_tab_();
        self.tab_widget.tabs[self.tab_widget.get_active_idx()].set_style(TabStyle::Normal);
    }

    pub(crate) fn prev_tab(&mut self) {
        self.prev_tab_();
        self.tab_widget.tabs[self.tab_widget.get_active_idx()].set_style(TabStyle::Normal);
    }

    pub(crate) fn switch(&mut self, string: &str) {
        let mut next_idx = self.tab_widget.get_active_idx();
        for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
            match tab.src {
                MsgSource::Serv { ref serv } => {
                    if serv.contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
                MsgSource::Chan { ref chan, .. } => {
                    if chan.contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
                MsgSource::User { ref nick, .. } => {
                    if nick.contains(string) {
                        next_idx = tab_idx;
                        break;
                    }
                }
            }
        }
        if next_idx != self.tab_widget.get_active_idx() {
            self.select_tab(next_idx);
        }
    }

    fn next_tab_(&mut self) {
        
    }

    fn prev_tab_(&mut self) {
        
    }

    fn move_tab_left(&mut self) {
        if self.tab_widget.get_active_idx() == 0 {
            return;
        }
        if self.is_server_tab(self.tab_widget.get_active_idx()) {
            // move all server tabs
            let (left, right) = self.server_tab_range(self.tab_widget.get_active_idx());
            if left > 0 {
                let mut insert_idx = left - 1;
                while insert_idx > 0 && !self.is_server_tab(insert_idx) {
                    insert_idx -= 1;
                }
                let to_move: Vec<Tab> = self.tab_widget.tabs.drain(left..right).collect();
                self.tab_widget.
                    tabs.splice(insert_idx..insert_idx, to_move.into_iter());
                self.select_tab(insert_idx);
            }
        } else if !self.is_server_tab(self.tab_widget.get_active_idx() - 1) {
            let tab = self
                .tab_widget
                .tabs
                .remove(self.tab_widget.get_active_idx());
            self.tab_widget
                .tabs
                .insert(self.tab_widget.get_active_idx() - 1, tab);
            let active_idx = self.tab_widget.get_active_idx() - 1;
            self.select_tab(active_idx);
        }
    }

    fn move_tab_right(&mut self) {
        if self.tab_widget.get_active_idx() == self.tab_widget.tabs.len() - 1 {
            return;
        }
        if self.is_server_tab(self.tab_widget.get_active_idx()) {
            // move all server tabs
            let (left, right) = self.server_tab_range(self.tab_widget.get_active_idx());
            if right < self.tab_widget.tabs.len() {
                let right_next = self.server_tab_range(right).1;
                let insert_idx = right_next - (right - left);
                let to_move: Vec<Tab> = self.tab_widget.tabs.drain(left..right).collect();
                self.tab_widget.
                    tabs.splice(insert_idx..insert_idx, to_move.into_iter());
                self.select_tab(insert_idx);
            }
        } else if !self.is_server_tab(self.tab_widget.get_active_idx() + 1) {
            let tab = self
                .tab_widget
                .tabs
                .remove(self.tab_widget.get_active_idx());
            self.tab_widget
                .tabs
                .insert(self.tab_widget.get_active_idx() + 1, tab);
            let active_idx = self.tab_widget.get_active_idx() + 1;
            self.select_tab(active_idx);
        }
    }

    ////////////////////////////////////////////////////////////////////////////
    // Interfacing with tabs

    fn apply_to_target<F>(&mut self, target: &MsgTarget, f: &F)
    where
        F: Fn(&mut Tab, bool),
    {
        // Creating a vector just to make borrow checker happy (I can't have a Vec<&mut Tab>)
        // I need to collect tabs here because of the "create if not exists" logic.
        // (see `target_idxs.is_empty()` below)
        let mut target_idxs: Vec<usize> = Vec::with_capacity(1);

        match *target {
            MsgTarget::Server { serv } => {
                for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
                    if let MsgSource::Serv { serv: ref serv_ } = tab.src {
                        if serv == serv_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::Chan { serv, chan } => {
                for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
                    if let MsgSource::Chan {
                        serv: ref serv_,
                        chan: ref chan_,
                    } = tab.src
                    {
                        if serv == serv_ && chan == chan_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::User { serv, nick } => {
                for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
                    if let MsgSource::User {
                        serv: ref serv_,
                        nick: ref nick_,
                    } = tab.src
                    {
                        if serv == serv_ && nick == nick_ {
                            target_idxs.push(tab_idx);
                            break;
                        }
                    }
                }
            }

            MsgTarget::AllServTabs { serv } => {
                for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
                    if tab.src.serv_name() == serv {
                        target_idxs.push(tab_idx);
                    }
                }
            }

            MsgTarget::CurrentTab => {
                target_idxs.push(self.tab_widget.get_active_idx());
            }
        }

        // Create server/chan/user tab when necessary
        if target_idxs.is_empty() {
            if let Some(idx) = self.maybe_create_tab(target) {
                target_idxs.push(idx);
            }
        }

        for tab_idx in target_idxs {
            f(
                &mut self.tab_widget.tabs[tab_idx],
                self.tab_widget.get_active_idx() == tab_idx,
            );
        }
    }

    fn maybe_create_tab(&mut self, target: &MsgTarget) -> Option<usize> {
        match *target {
            MsgTarget::Server { serv } | MsgTarget::AllServTabs { serv } => {
                self.new_server_tab(serv, None)
            }

            MsgTarget::Chan { serv, chan } => self.new_chan_tab(serv, chan),

            MsgTarget::User { serv, nick } => self.new_user_tab(serv, nick),

            _ => None,
        }
    }

    pub(crate) fn set_tab_style(&mut self, style: TabStyle, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, is_active: bool| {
            if !is_active && tab.style < style {
                tab.set_style(style);
            }
        });
    }

    /// An error message coming from Tiny, probably because of a command error
    /// etc. Those are not timestamped and not logged.
    pub(crate) fn add_client_err_msg(&mut self, msg: &str, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_client_err_msg(msg);
        });
    }

    /// A notify message coming from tiny, usually shows a response of a command
    /// e.g. "Notifications enabled".
    pub(crate) fn add_client_notify_msg(&mut self, msg: &str, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_client_notify_msg(msg);
        });
    }

    /// A message from client, usually just to indidate progress, e.g.
    /// "Connecting...". Not timestamed and not logged.
    pub(crate) fn add_client_msg(&mut self, msg: &str, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_client_msg(msg);
        });
    }

    /// privmsg is a message coming from a server or client. Shown with sender's
    /// nick/name and receive time and logged.
    pub(crate) fn add_privmsg(
        &mut self,
        sender: &str,
        msg: &str,
        ts: Tm,
        target: &MsgTarget,
        highlight: bool,
        is_action: bool,
    ) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget
                .add_privmsg(sender, msg, Timestamp::from(ts), highlight, is_action);
            let nick = tab.widget.get_nick();
            if let Some(nick_) = nick {
                tab.notifier
                    .notify_privmsg(sender, msg, target, &nick_, highlight);
            }
        });
    }

    /// A message without any explicit sender info. Useful for e.g. in server
    /// and debug log tabs. Timestamped and logged.
    pub(crate) fn add_msg(&mut self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_msg(msg, Timestamp::from(ts));
        });
    }

    /// Error messages related with the protocol - e.g. can't join a channel,
    /// nickname is in use etc. Timestamped and logged.
    pub(crate) fn add_err_msg(&mut self, msg: &str, ts: Tm, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.add_err_msg(msg, Timestamp::from(ts));
        });
    }

    pub(crate) fn set_topic(&mut self, title: &str, ts: Tm, serv: &str, chan: &str) {
        let target = MsgTarget::Chan { serv, chan };
        self.apply_to_target(&target, &|tab: &mut Tab, _| {
            tab.widget.show_topic(title, Timestamp::from(ts));
        });
    }

    pub(crate) fn clear_nicks(&mut self, serv: &str) {
        let target = MsgTarget::AllServTabs { serv };
        self.apply_to_target(&target, &|tab: &mut Tab, _| {
            tab.widget.clear_nicks();
        });
    }

    pub(crate) fn add_nick(&mut self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.join(nick, ts.map(Timestamp::from));
        });
    }

    pub(crate) fn remove_nick(&mut self, nick: &str, ts: Option<Tm>, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.part(nick, ts.map(Timestamp::from));
        });
    }

    pub(crate) fn rename_nick(
        &mut self,
        old_nick: &str,
        new_nick: &str,
        ts: Tm,
        target: &MsgTarget,
    ) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.widget.nick(old_nick, new_nick, Timestamp::from(ts));
            // TODO: Does this actually rename the tab?
            tab.update_source(&|src: &mut MsgSource| {
                if let MsgSource::User { ref mut nick, .. } = *src {
                    nick.clear();
                    nick.push_str(new_nick);
                }
            });
        });
    }

    pub(crate) fn set_nick(&mut self, serv: &str, new_nick: &str) {
        let target = MsgTarget::AllServTabs { serv };
        self.apply_to_target(&target, &|tab: &mut Tab, _| {
            tab.widget.set_nick(new_nick.to_owned())
        });
    }

    pub(crate) fn clear(&mut self, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| tab.widget.clear());
    }

    pub(crate) fn toggle_statusline(&mut self) {
        self.show_statusline = !self.show_statusline;
        self.resize();
    }

    pub(crate) fn toggle_ignore(&mut self, target: &MsgTarget) {
        if let MsgTarget::AllServTabs { serv } = *target {
            let mut status_val: bool = false;
            for tab in &self.tab_widget.tabs {
                if let MsgSource::Serv { serv: ref serv_ } = tab.src {
                    if serv == serv_ {
                        status_val = tab.widget.get_ignore_state();
                        break;
                    }
                }
            }
            self.apply_to_target(target, &|tab: &mut Tab, _| {
                tab.widget.set_or_toggle_ignore(Some(!status_val));
            });
        } else {
            self.apply_to_target(target, &|tab: &mut Tab, _| {
                tab.widget.set_or_toggle_ignore(None);
            });
        }
    }

    // TODO: Maybe remove this and add a `create: bool` field to MsgTarget::User
    pub(crate) fn user_tab_exists(&self, serv_: &str, nick_: &str) -> bool {
        for tab in &self.tab_widget.tabs {
            if let MsgSource::User { ref serv, ref nick } = tab.src {
                if serv_ == serv && nick_ == nick {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn set_notifier(&mut self, notifier: Notifier, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            tab.notifier = notifier;
        });
    }

    pub(crate) fn show_notify_mode(&mut self, target: &MsgTarget) {
        self.apply_to_target(target, &|tab: &mut Tab, _| {
            let msg = match tab.notifier {
                Notifier::Off => "Notifications are off",
                Notifier::Mentions => "Notifications enabled for mentions",
                Notifier::Messages => "Notifications enabled for all messages",
            };
            tab.widget.add_client_notify_msg(msg);
        });
    }

    ////////////////////////////////////////////////////////////////////////////
    // Helpers

    fn find_serv_tab_idx(&self, serv_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
            if let MsgSource::Serv { ref serv } = tab.src {
                if serv_ == serv {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_chan_tab_idx(&self, serv_: &str, chan_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
            if let MsgSource::Chan { ref serv, ref chan } = tab.src {
                if serv_ == serv && chan_ == chan {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_user_tab_idx(&self, serv_: &str, nick_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate() {
            if let MsgSource::User { ref serv, ref nick } = tab.src {
                if serv_ == serv && nick_ == nick {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    /// Index of the last tab with the given server name.
    fn find_last_serv_tab_idx(&self, serv: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tab_widget.tabs.iter().enumerate().rev() {
            if tab.src.serv_name() == serv {
                return Some(tab_idx);
            }
        }
        None
    }

    fn is_server_tab(&self, idx: usize) -> bool {
        match self.tab_widget.tabs[idx].src {
            MsgSource::Serv { .. } => true,
            MsgSource::Chan { .. } | MsgSource::User { .. } => false,
        }
    }

    /// Given a tab index return range of tabs for the server of this tab.
    fn server_tab_range(&self, idx: usize) -> (usize, usize) {
        debug_assert!(idx < self.tab_widget.tabs.len());
        let mut left = idx;
        while !self.is_server_tab(left) {
            left -= 1;
        }
        let mut right = idx + 1;
        while right < self.tab_widget.tabs.len() && !self.is_server_tab(right) {
            right += 1;
        }
        (left, right)
    }
}
