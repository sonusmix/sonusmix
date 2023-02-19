use yew::prelude::*;
use yewprint::{Button, Intent};
use csscolorparser::Color;
use stylist::css;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub intent: Option<Intent>,
    pub active_color: Option<Color>,
    pub onchange: Callback<bool>,
    pub children: Children,
}

#[function_component(ToggleButton)]
pub fn toggle_button(props: &Props) -> Html {
    let active = use_state_eq(|| false);
    let onclick = {
        let active = active.clone();
        let onchange = props.onchange.clone();
        Callback::from(move |_| {
            let state = !*active;
            active.set(state);
            onchange.emit(state);
        })
    };

    html! {
        <Button
            onclick={ onclick }
            active={ *active }
            intent={ props.intent }
            class={ classes!(props.active_color.as_ref()
                .map(|color| css!(":active, &.bp3-active { background-color: ${color} !important; }", color=color.to_hex_string()))
            )}
        >
            { for props.children.iter() }
        </Button>
    }
}