use gio::prelude::*;
use gtk::prelude::*;
use libtiny_ui::*;
use tokio::sync::mpsc;

mod tabs;
mod messaging;

use tabs::Tabs;

#[derive(Debug)]
enum MsgTargetOwned {
    Server { serv: String },
    Chan { serv: String, chan: String },
    User { serv: String, nick: String },
    AllServTabs { serv: String },
    CurrentTab,
}

impl MsgTargetOwned {
    fn from(t: &MsgTarget) -> MsgTargetOwned {
        use MsgTargetOwned::*;
        match t {
            MsgTarget::Server { serv } => Server {
                serv: serv.to_string(),
            },
            MsgTarget::Chan { serv, chan } => Chan {
                serv: serv.to_string(),
                chan: chan.to_string(),
            },
            MsgTarget::User { serv, nick } => User {
                serv: serv.to_string(),
                nick: nick.to_string(),
            },
            MsgTarget::AllServTabs { serv } => AllServTabs {
                serv: serv.to_string(),
            },
            MsgTarget::CurrentTab => CurrentTab,
        }
    }

    fn borrow(&self) -> MsgTarget {
        use MsgTargetOwned::*;
        match self {
            Server { ref serv } => MsgTarget::Server { serv },
            Chan { ref serv, ref chan } => MsgTarget::Chan { serv, chan },
            User { ref serv, ref nick } => MsgTarget::User { serv, nick },
            AllServTabs { ref serv } => MsgTarget::AllServTabs { serv },
            CurrentTab => MsgTarget::CurrentTab,
        }
    }
}

fn main() {
    let application = gtk::Application::new(Some("com.github.osa1.tiny"), Default::default())
        .expect("Initialization failed...");

    let (snd_ev, rcv_ev) = mpsc::channel::<Event>(1000);

    application.connect_activate(move |app| {
        build_ui(app, snd_ev.clone());
    });

    application.run(&std::env::args().collect::<Vec<_>>());
}

fn build_ui(
    application: &gtk::Application,
    snd_ev: mpsc::Sender<Event>,
) {
    let mut tabs = Tabs::new(snd_ev);
    tabs.new_server_tab("mentions".to_string());

    let window = gtk::ApplicationWindow::new(application);

    window.set_title("tiny");
    window.set_decorated(false);
    window.set_default_size(200, 200);
    window.add(tabs.get_widget());
    window.show_all();
}
