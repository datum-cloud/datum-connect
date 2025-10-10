use dioxus::prelude::*;

use crate::Route;

#[derive(PartialEq, Clone, Props)]
pub struct ButtonProps {
    text: String,
    to: Option<Route>,
    onclick: Option<EventHandler<MouseEvent>>,
}

const CLASS: &str = "py-2 px-5 border-1 border-white rounded-md";

#[component]
pub fn Button(props: ButtonProps) -> Element {
    match (props.to.is_some(), props.onclick.is_some()) {
        (true, false) => {
            rsx! {
                Link {
                    to: props.to.unwrap().to_string(),
                    class: CLASS,
                    {props.text}
                }
            }
        }
        (false, true) => {
            rsx! {
                button {
                    class: CLASS,
                    onclick: move |evt| props.onclick.unwrap().call(evt),
                    {props.text}
                }
            }
        }
        _ => {
            rsx! {
                button {
                    class: CLASS,
                    {props.text}
                }
            }
        }
    }
}

#[derive(PartialEq, Clone, Props)]
pub struct CloseButtonProps {
    onclick: EventHandler<MouseEvent>,
}

#[component]
pub fn CloseButton(props: CloseButtonProps) -> Element {
    rsx! {
        button {
            class: "border-1 w-6 h-6 rounded-full pb-0.5 text-xs text-gray-500 hover:text-red-500 cursor-pointer",
            onclick: move |evt| props.onclick.call(evt),
            "x"
        }
    }
}
