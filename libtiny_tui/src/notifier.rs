use crate::{utils::remove_irc_control_chars, MsgTarget};
use notify_rust::Notification;
use libtiny_ui::Notifier;

fn notify(summary: &str, body: &str) {
    // TODO: Report errors somehow
    let _ = Notification::new().summary(summary).body(body).show();
}

pub(crate) fn notify_privmsg(
    notifier: Notifier,
    sender: &str,
    msg: &str,
    target: &MsgTarget,
    our_nick: &str,
    mention: bool,
) {
    if our_nick == sender {
        return;
    }

    let msg = remove_irc_control_chars(msg);

    match *target {
        MsgTarget::Chan { chan, .. } => {
            if notifier == Notifier::Messages || (notifier == Notifier::Mentions && mention) {
                notify(&format!("{} in {}", sender, chan), &msg)
            }
        }
        MsgTarget::User {
            nick: ref nick_sender,
            ..
        } => {
            if notifier != Notifier::Off {
                notify(&format!("{} sent a private message", nick_sender), &msg)
            }
        }
        _ => {}
    }
}
