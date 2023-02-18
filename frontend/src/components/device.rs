use csscolorparser::Color;
use implicit_clone::unsync::IArray;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use stylist::css;
use yewprint::{Card, Elevation, Text, Slider};

use super::ToggleButton;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub device_name: AttrValue,
    #[prop_or(Color::new(1.0, 1.0, 1.0, 1.0))]
    pub color: Color,
    #[prop_or_default]
    pub buttons: IArray<AttrValue>,
}

#[function_component(Device)]
pub fn device(props: &Props) -> Html {
    let volume = use_state_eq(|| 0);
    let slider_onchange = {
        let volume = volume.clone();
        Callback::from(move |val| volume.set(val))
    };
    // let slider_hover = use_state_eq(|| false);
    // let slider_mouseover = {
    //     let slider_hover = slider_hover.clone();
    //     Callback::from(move |_| slider_hover.set(true))
    // };
    // let slider_mouseout = {
    //     let slider_hover = slider_hover.clone();
    //     Callback::from(move |_| slider_hover.set(false))
    // };

    let color = use_state_eq(|| props.color.clone());
    let color_picker_change = {
        let color = color.clone();
        Callback::from(move |e: InputEvent| color.set(
            csscolorparser::parse(&e.target_unchecked_into::<HtmlInputElement>().value()).unwrap_or_default()
        ))
    };

    html! {
        <Card
            elevation={ Elevation::Level1 }
            class={ css!("background-color: ${color} !important; padding: 5px; margin: 15px; ", color=color.to_hex_string()) }
        >
            <Card elevation={ Elevation::Level0 }>
                <Text class="bp3-heading" >{ &props.device_name }</Text>
                <div
                    onmousedown={ |e: MouseEvent| e.prevent_default() }
                >
                    <Slider<u32>
                        onchange={ slider_onchange }
                        values={ (0..=100).map(|i| (i, None)).collect::<Vec<_>>() }
                        selected={ *volume }
                        value_label={ format!("{}%", *volume) }
                    />
                </div>
                {
                    props.buttons.iter().map(|text| {
                        html! { <ToggleButton onchange={ |_| () }>{ text }</ToggleButton> }
                    }).collect::<Html>()
                }
                <input type="color" value={ color.to_hex_string() } oninput={ color_picker_change } class={ css!(float: right;) } />
            </Card>
        </Card>
    }
}