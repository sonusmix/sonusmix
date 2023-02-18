use yew::prelude::*;
use yewprint::Button;

#[derive(Properties, PartialEq)]
pub struct Props {
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
        <Button onclick={ onclick } active={ *active }>{ for props.children.iter() }</Button>
    }
}