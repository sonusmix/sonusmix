use std::cell::RefCell;

use ksni::{menu::*, *};

use crate::{StatusMsg, SONUSMIX_APP_ID};

#[derive(Debug)]
pub struct SonusmixTray {
    tx: RefCell<relm4::Sender<StatusMsg>>,
    should_exit: bool,
}

impl SonusmixTray {
    pub fn new() -> (Self, relm4::Receiver<StatusMsg>) {
        let (tx, rx) = relm4::channel();
        (
            Self {
                tx: RefCell::new(tx),
                should_exit: false,
            },
            rx,
        )
    }

    pub fn status(&self) -> relm4::Receiver<StatusMsg> {
        let mut lock = self.tx.borrow_mut();
        let (tx, rx) = relm4::channel();
        if self.should_exit {
            let _ = tx.send(StatusMsg::Exit);
        }
        *lock = tx;
        rx
    }
}

impl Tray for SonusmixTray {
    fn category(&self) -> Category {
        Category::ApplicationStatus
    }
    fn icon_name(&self) -> String {
        SONUSMIX_APP_ID.to_owned()
    }
    fn id(&self) -> String {
        SONUSMIX_APP_ID.to_owned()
    }
    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "Sonusmix is running".to_owned(),
            ..Default::default()
        }
    }
    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.borrow().send(StatusMsg::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Exit".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    tray.should_exit = true;
                    let _ = tray.tx.borrow().send(StatusMsg::Exit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
