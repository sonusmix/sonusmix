use std::sync::atomic::Ordering;

use ksni::{menu::*, *};

use crate::{MainMsg, APP_WINDOW_ID, SONUSMIX_APP_ID};

#[derive(Debug)]
pub struct SonusmixTray {
    sender: relm4::Sender<MainMsg>,
}

impl SonusmixTray {
    pub fn new(sender: relm4::Sender<MainMsg>) -> Self {
        Self { sender }
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
    fn window_id(&self) -> i32 {
        APP_WINDOW_ID.load(Ordering::Acquire)
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
                    let _ = tray.sender.send(MainMsg::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Hide".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send(MainMsg::Hide);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Exit".to_owned(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.sender.send(MainMsg::Exit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}
