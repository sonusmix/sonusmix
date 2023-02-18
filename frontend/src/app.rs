use implicit_clone::unsync::IArray;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use csscolorparser::Color;

use crate::components::Device;

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

    let button_names = ["Headset", "Headphones", "Speakers"].into_iter()
        .map(AttrValue::from)
        .collect::<IArray<AttrValue>>();

    html! {
        <main class="container bp3-dark">
            <Device device_name="Headset Mic" buttons={ button_names.clone() } color={ Color::new(0.0, 0.0, 0.0, 1.0) } />
            <Device device_name="Webcam Mic" buttons={ button_names.clone() } color={ Color::new(0.0, 0.0, 0.0, 1.0) } />
            <Device device_name="Discord" buttons={ button_names.clone() } color={ Color::new(0.345, 0.396, 0.949, 1.0) } />
        </main>
    }
}
