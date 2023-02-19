use implicit_clone::unsync::IArray;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use csscolorparser::Color;
use stylist::css;

use crate::components::{Device, SplitPanes};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;

    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Serialize, Deserialize)]
struct GreetArgs<'a> {
    name: &'a str,
}

#[function_component(App)]
pub fn app() -> Html {

    let input_devices = ["Headset Mic", "Webcam Mic", "Discord"].into_iter()
        .map(AttrValue::from)
        .collect::<IArray<AttrValue>>();

    let output_devices = ["Headset", "Headphones", "Speakers", "Discord"].into_iter()
        .map(AttrValue::from)
        .collect::<IArray<AttrValue>>();

    html! {
        <main class={ classes!("container", "bp3-dark", css!(height: 100%;)) }>
            <SplitPanes
                class={ css!(height: 100%;) }
                left_children={ input_devices.iter()
                    .map(|name| {
                        html! { <Device device_name={ name.clone() } buttons={ output_devices.clone() } color={ if name == "Discord" {
                                Color::new(0.345, 0.396, 0.949, 1.0)
                            } else {
                                Color::new(0.0, 0.0, 0.0, 1.0)
                            }
                        } /> }
                    })
                    .collect::<Html>()
                }
                right_children={ output_devices.iter()
                    .map(|name| {
                        html! { <Device device_name={ name.clone() } buttons={ input_devices.clone() } color={ if name == "Discord" {
                                Color::new(0.345, 0.396, 0.949, 1.0)
                            } else {
                                Color::new(0.0, 0.0, 0.0, 1.0)
                            }
                        } /> }
                    })
                    .collect::<Html>()
                }
            />
        </main>
    }
}
