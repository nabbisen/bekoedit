use dioxus::prelude::*;

#[component]
pub fn FolderIcon() -> Element {
    rsx! {
        svg {
            class: "ui-icon",
            view_box: "0 0 24 24",
            width: "18",
            height: "18",
            "aria-hidden": "true",
            path {
                d: "M3 6.5h6l2 2h10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.8",
                stroke_linejoin: "round",
            }
        }
    }
}

#[component]
pub fn NewFileIcon() -> Element {
    rsx! {
        svg {
            class: "ui-icon",
            view_box: "0 0 24 24",
            width: "18",
            height: "18",
            "aria-hidden": "true",
            path {
                d: "M6 3.5h8l4 4v13H6zM14 3.5v4h4M12 11v6M9 14h6",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "1.8",
                stroke_linecap: "round",
                stroke_linejoin: "round",
            }
        }
    }
}

#[component]
pub fn AddIcon() -> Element {
    rsx! {
        svg {
            class: "ui-icon",
            view_box: "0 0 24 24",
            width: "18",
            height: "18",
            "aria-hidden": "true",
            path {
                d: "M12 5v14M5 12h14",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
            }
        }
    }
}
