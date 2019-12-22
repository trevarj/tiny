use crate::{MsgSource, TabStyle, Notifier};

use std::collections::HashMap;

pub struct Tab<W> {
    pub widget: W,
    pub src: MsgSource,
    pub style: TabStyle,
    pub switch: Option<char>,
    pub notifier: Notifier,
}

pub struct Tabs<W> {
    pub tabs: Vec<Tab<W>>,
}

impl<W> Tabs<W> {
    pub fn new_server_tab(&mut self, serv: String) -> Option<usize> {
        match self.find_serv_tab_idx(&serv) {
            None => {
                let tab_idx = self.tabs.len();
                self.new_tab(
                    tab_idx,
                    MsgSource::Serv {
                        serv: serv.to_owned(),
                    },
                    true,
                    Notifier::Mentions,
                );
                Some(tab_idx)
            }
            Some(_) => None,
        }
    }

    fn new_tab(&mut self, idx: usize, src: MsgSource, show_statusline: bool, notifier: Notifier) {
        let src_ = src.clone();
        // let msg_ui = MessagingUI::new(src_, self.snd_ev.clone());
        // let label_str = match src {
        //     MsgSource::Serv { ref serv } => serv,
        //     MsgSource::Chan { ref chan, .. } => chan,
        //     MsgSource::User { ref nick, .. } => nick,
        // };
        // let label = gtk::Label::new(Some(label_str));
        // self.notebook
        //     .insert_page(msg_ui.get_widget(), Some(&label), Some(idx as u32));

        let mut switch_keys: HashMap<char, i8> = HashMap::with_capacity(self.tabs.len());
        for tab in &self.tabs {
            if let Some(key) = tab.switch {
                switch_keys.entry(key).and_modify(|e| *e += 1).or_insert(1);
            }
        }

        let switch = {
            let mut ret = None;
            let mut n = 0;
            for ch in src.visible_name().chars() {
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

        self.tabs.insert(
            idx,
            Tab {
                widget,
                src,
                style: TabStyle::Normal,
                switch, 
                notifier,
            },
        );
        // self.notebook.show_all();
    }

    fn find_serv_tab_idx(&self, serv_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::Serv { ref serv } = tab.src {
                if serv_ == serv {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_chan_tab_idx(&self, serv_: &str, chan_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
            if let MsgSource::Chan { ref serv, ref chan } = tab.src {
                if serv_ == serv && chan_ == chan {
                    return Some(tab_idx);
                }
            }
        }
        None
    }

    fn find_user_tab_idx(&self, serv_: &str, nick_: &str) -> Option<usize> {
        for (tab_idx, tab) in self.tabs.iter().enumerate() {
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
        for (tab_idx, tab) in self.tabs.iter().enumerate().rev() {
            if tab.src.serv_name() == serv {
                return Some(tab_idx);
            }
        }
        None
    }
}
