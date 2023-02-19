use implicit_clone::{unsync::IArray, ImplicitClone};
use web_sys::{Element};
use yew::prelude::*;
use stylist::css;
use yewprint::{Tabs, Tab};
use gloo::{console::log, utils::window, events::EventListener};

#[derive(Properties, PartialEq)]
pub struct Props {
    #[prop_or_default]
    pub class: Classes,
    #[prop_or(0.5)]
    pub split: f32,
    #[prop_or(300.0)]
    pub min_pane_width: f32,
    #[prop_or(800.0)]
    pub min_split_width: f32,
    pub left_title: AttrValue,
    pub right_title: AttrValue,
    pub left_children: Children,
    pub right_children: Children,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    Left,
    Right,
}

impl ImplicitClone for PaneId { }

#[function_component(Panes)]
pub fn panes(props: &Props) -> Html {
    // TODO: Find a way to make internals not re-render when changing layouts

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
    });

    let on_resize = {
        let container_width = container_width.clone();
        let outer_div_ref = outer_div_ref.clone();
        Callback::from(move |_| {
            if let Some(element) = outer_div_ref.cast::<Element>() {
                container_width.set(element.client_width() as f32);
            }
        })
    };

    use_effect_with_deps({
        move |_| {
            let listener = EventListener::new(&window(), "resize", move |event| {
                on_resize.emit(event.clone());
            });
            move || std::mem::drop(listener)
        }
    }, ());

    let selected_tab = use_state_eq(|| PaneId::Left);

    let change_tab = {
        let selected_tab = selected_tab.clone();
        Callback::from(move |tab| selected_tab.set(tab))
    };

    let left_children = props.left_children.iter().collect::<Html>();
    let right_children = props.right_children.iter().collect::<Html>();

    html! {
        <div
            ref={ outer_div_ref }
            onmousemove={ mouse_move }
            onmouseup={ stop_dragging.clone() }
            onmouseleave={ stop_dragging }
            class={ classes!(css!(display: flex; width: 100%; cursor: ${if *dragging { "ew-resize" } else { "auto"}};), props.class.clone()) }
        >{
            if *container_width >= props.min_split_width {
                html! { <>
                    <div style={ format!("width: {}px;", *container_width * *split) } class={ css!(min-width: ${props.min_pane_width}px; overflow: scroll;) }>
                        <PaneTitle title={ &props.left_title } />
                        { left_children }
                    </div>
                    <div onmousedown={ start_dragging } class={ css!(cursor: ew-resize; user-select: none; width: 6px; background-color: var(--ui-recessed-color); border-radius: 3px; ) }></div>
                    <div class={ css!(flex: 1 1 0%; min-width: ${props.min_pane_width}px; overflow: scroll;) }>
                        <PaneTitle title={ &props.right_title } />
                        { right_children }
                    </div>
                </> }
            } else {
                html! {
                    <Tabs<PaneId>
                        animate=true
                        large=true
                        id="panes-tabs"
                        onchange={ change_tab }
                        selected_tab_id={ *selected_tab }
                        tabs={ [
                            Tab {
                                disabled: false,
                                id: PaneId::Left,
                                title: html! { <strong>{ &props.left_title }</strong> },
                                title_class: Classes::default(),
                                panel: left_children,
                                panel_class: css!(overflow: scroll; margin-top: 0px;).into(),
                            },
                            Tab {
                                disabled: false,
                                id: PaneId::Right,
                                title: html! { <strong>{ &props.right_title }</strong> },
                                title_class: Classes::default(),
                                panel: right_children,
                                panel_class: css!(overflow: scroll; margin-top: 0px;).into(),
                            }
                        ].into_iter().collect::<IArray<_>>() }
                        class={ css!(width: 100%; min-width: ${props.min_pane_width}; > .bp3-tab-list { margin-left: 20px; }) }
                    />
                }
            }
        }</div>
    }
}

#[derive(Properties, PartialEq)]
struct PaneTitleProps {
    title: AttrValue,
}

#[function_component(PaneTitle)]
fn pane_title(props: &PaneTitleProps) -> Html {
    html! {
        <div class={ css!(height: 40px; margin-left: 20px; border-bottom: 3px solid var(--ui-text-color); width: max-content;) }>
            <strong class={ css!(font-size: 16px; line-height: 40px;) }>{ &props.title }</strong>
        </div>
    }
}