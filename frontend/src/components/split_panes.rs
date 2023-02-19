use web_sys::{Element};
use yew::prelude::*;
use stylist::css;

#[derive(Properties, PartialEq)]
pub struct Props {
    #[prop_or_default]
    pub class: Classes,
    #[prop_or(0.5)]
    pub split: f32,
    pub left_children: Children,
    pub right_children: Children,
}

#[function_component(SplitPanes)]
pub fn split_panes(props: &Props) -> Html {
    let dragging = use_state_eq(|| false);
    let split = use_state_eq(|| props.split);
    let container_width = use_state_eq(|| 0f32);
    let outer_div_ref = use_node_ref();

    let start_dragging = {
        let dragging = dragging.clone();
        Callback::from(move |event: MouseEvent| {
            dragging.set(true);
            event.prevent_default()
        })
    };

    let stop_dragging = {
        let dragging = dragging.clone();
        Callback::from(move |_| {
            dragging.set(false);
        })
    };

    let mouse_move = {
        let dragging = dragging.clone();
        let container_width = container_width.clone();
        let split = split.clone();
        Callback::from(move |event: MouseEvent| {
            if *dragging {
                split.set(event.client_x() as f32 / *container_width);
            }
        })
    };

    use_effect({
            let container_width = container_width.clone();
            let outer_div_ref = outer_div_ref.clone();
            move || {
                if let Some(element) = outer_div_ref.cast::<Element>() {
                    container_width.set(element.client_width() as f32);
                }
            }
        },
    );

    html! {
        <div
            ref={ outer_div_ref }
            onmousemove={ mouse_move }
            onmouseup={ stop_dragging.clone() }
            onmouseleave={ stop_dragging }
            class={ classes!(css!(display: flex; width: 100%; ), props.class.clone()) }
        >
            <div style={ format!("width: {}px;", *container_width * *split) } class={ css!(min-width: 250px; overflow: scroll;) }>
                { for props.left_children.iter() }
            </div>
            <div onmousedown={ start_dragging } class={ css!(cursor: ew-resize; user-select: none; width: 6px; background-color: var(--ui-recessed-color); border-radius: 3px; ) }></div>
            <div  class={ css!(flex: 1 1 0%; min-width: 250px; overflow: scroll;) }>{ for props.right_children.iter() }</div>
        </div>
    }
}