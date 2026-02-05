use crate::{
    Route, components::{
        AddTunnelDialog, Button, ButtonKind, Icon, IconSource, InviteUserDialog, dropdown_menu::{
            DropdownAlign, DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger, DropdownSide
        }, select::{Select, SelectAlign, SelectItemIndicator, SelectList, SelectOptionItem, SelectSide, SelectSize, SelectTrigger, SelectValue}
    }, state::AppState
};
use lib::datum_cloud::{LoginState, OrganizationWithProjects};
use dioxus::prelude::*;
use open::that;

/// Provided by Sidebar so child routes (e.g. TunnelBandwidth) can open the Add/Edit tunnel dialog.
#[derive(Clone)]
pub struct OpenEditTunnelDialog {
    pub editing_proxy: Signal<Option<lib::ProxyState>>,
    pub editing_tunnel: Signal<Option<lib::TunnelSummary>>,
    pub dialog_open: Signal<bool>,
}

#[component]
pub fn Chrome() -> Element {
    rsx! {
        div { class: "h-screen overflow-hidden flex flex-col bg-content-background text-foreground border-t border-app-border",
            Outlet::<Route> {}
        }
    }
}

#[component]
pub fn Sidebar() -> Element {
    let auth_changed = consume_context::<Signal<u32>>();
    let _ = auth_changed();
    let auth_state = consume_context::<AppState>().datum().auth_state();
    let mut profile_menu_open = use_signal(|| None::<bool>);
    let nav = use_navigator();
    let state = consume_context::<AppState>();
    let mut add_tunnel_dialog_open = use_signal(|| false);
    let mut invite_user_dialog_open = use_signal(|| false);
    let editing_proxy = use_signal(|| None::<lib::ProxyState>);
    let mut editing_tunnel = use_signal(|| None::<lib::TunnelSummary>);

    let mut selected_org_id = use_signal(|| state.selected_context().map(|c| c.org_id));
    let mut selected_project_id = use_signal(|| state.selected_context().map(|c| c.project_id));
    let mut pending_org_switch = use_signal(|| false);
    let orgs = use_signal(Vec::<OrganizationWithProjects>::new);
    let state_for_orgs = state.clone();
    let orgs_for_future = orgs.clone();
    use_future(move || {
        let state_for_orgs = state_for_orgs.clone();
        let mut orgs = orgs_for_future.clone();
        async move {
            if state_for_orgs.datum().login_state() != LoginState::Valid {
                return;
            }
            if let Ok(list) = state_for_orgs.datum().orgs_and_projects().await {
                orgs.set(list);
            }
        }
    });
    let orgs_snapshot = orgs.read().clone();
    let selected_org_snapshot = selected_org_id.read().clone();
    let selected_ctx = state.selected_context();
    let org_options: Vec<(String, String)> = if orgs_snapshot.is_empty() {
        selected_ctx
            .as_ref()
            .map(|ctx| vec![(ctx.org_id.clone(), ctx.org_name.clone())])
            .unwrap_or_default()
    } else {
        orgs_snapshot
            .iter()
            .map(|org| (org.org.resource_id.clone(), org.org.display_name.clone()))
            .collect()
    };
    let project_options: Vec<(String, String)> = if orgs_snapshot.is_empty() {
        selected_ctx
            .as_ref()
            .map(|ctx| vec![(ctx.project_id.clone(), ctx.project_name.clone())])
            .unwrap_or_default()
    } else {
        selected_org_snapshot
            .as_ref()
            .and_then(|org_id| {
                orgs_snapshot.iter().find(|org| &org.org.resource_id == org_id)
            })
            .map(|org| {
                org.projects
                    .iter()
                    .map(|p| (p.resource_id.clone(), p.display_name.clone()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };

    provide_context(OpenEditTunnelDialog {
        editing_proxy,
        editing_tunnel,
        dialog_open: add_tunnel_dialog_open,
    });

    let state_for_effect = state.clone();
    use_effect(move || {
        if state_for_effect.datum().login_state() == LoginState::Missing {
            nav.push(Route::Login {});
            return;
        }
        if state_for_effect.selected_context().is_none() {
            nav.push(Route::SelectProject {});
        }
    });

    let user_name = match auth_state.get() {
        Ok(auth) => auth.profile.display_name(),
        Err(_) => "Not logged in".to_string(),
    };
    let user_email = match auth_state.get() {
        Ok(auth) => auth.profile.email.clone(),
        Err(_) => "Not logged in".to_string(),
    };
    let avatar_url = match auth_state.get() {
        Ok(auth) => auth.profile.avatar_url.clone(),
        Err(_) => None,
    };
    let mut logout = use_action(move |_: ()| {
        let mut auth_changed = auth_changed.clone();
        async move {
            let state = consume_context::<AppState>();
            state.datum().auth().logout().await?;
            auth_changed.set(auth_changed() + 1);
            nav.push(Route::Login {});
            n0_error::Ok(())
        }
    });

    let route = use_route::<Route>();
    let sidebar_hidden = matches!(route, Route::TunnelBandwidth { .. });
    let sidebar_class = if sidebar_hidden {
        "min-w-[230px] max-w-[230px] shrink-0 flex-none bg-background border-r border-app-border pt-5 pb-6 px-4 flex flex-col absolute left-0 -translate-x-[230px] z-10"
    } else {
        "min-w-[230px] max-w-[230px] shrink-0 flex-none bg-background border-r border-app-border pt-5 pb-6 px-4 flex flex-col"
    };

    let sidebar = rsx! {
        // Sidebar
        div { class: "{sidebar_class}",
            // Full-width content with equal left/right padding
            div { class: "w-full",
                Button {
                    leading_icon: Some(IconSource::Named("plus".into())),
                    text: "Add tunnel",
                    kind: ButtonKind::Primary,
                    class: "w-full",
                    onclick: move |_| add_tunnel_dialog_open.set(true),
                }
            }

            // Bottom nav (visual-only for now)
            div { class: "w-full mt-auto space-y-4 pl-2",
                div {
                    class: "flex items-center gap-3 cursor-pointer hover:opacity-80 duration-300 text-foreground text-xs",
                    onclick: move |_| {
                        let _ = that("https://www.datum.net/docs/");
                    },
                    Icon {
                        source: IconSource::Named("book-open".into()),
                        size: 16,
                        class: "text-icon-select",
                    }
                    span { class: "text-xs leading-4", "Docs" }
                }
                div {
                    class: "flex items-center gap-3 cursor-default hover:opacity-80 duration-300 text-foreground text-xs",
                    onclick: move |_| invite_user_dialog_open.set(true),
                    Icon {
                        source: IconSource::Named("users".into()),
                        size: 16,
                        class: "text-icon-select",
                    }
                    span { class: "text-xs", "Invite" }
                }
                div {
                    class: "flex items-center gap-3 cursor-default hover:opacity-80 duration-300 text-foreground text-xs",
                    onclick: move |_| {
                        let _ = nav.push(Route::Settings {});
                    },
                    Icon {
                        source: IconSource::Named("settings".into()),
                        size: 16,
                        class: "text-icon-select",
                    }
                    span { class: "text-xs", "Settings" }
                }
            }

            if auth_state.get().is_ok() {
                div { class: "mt-4 relative",
                    DropdownMenu {
                        open: profile_menu_open,
                        default_open: false,
                        on_open_change: move |v| profile_menu_open.set(Some(v)),
                        disabled: use_signal(|| false),
                        DropdownMenuTrigger { class: "flex items-center gap-1 justify-start focus:outline-2 focus:outline-app-border/50",
                            div { class: "w-9 h-9 rounded-lg border border-app-border bg-white flex items-center justify-center cursor-default overflow-hidden",
                                if avatar_url.is_some() {
                                    img {
                                        src: "{avatar_url.as_deref().unwrap_or(\"\")}",
                                        class: "object-cover w-full h-full",
                                    }
                                } else {
                                    svg {
                                        width: "18",
                                        height: "18",
                                        view_box: "0 0 24 24",
                                        fill: "none",
                                        path {
                                            d: "M12 12a4 4 0 1 0-4-4 4 4 0 0 0 4 4Z",
                                            stroke: "currentColor",
                                            stroke_width: "1.6",
                                        }
                                        path {
                                            d: "M4 21c1.6-3.5 4.6-5 8-5s6.4 1.5 8 5",
                                            stroke: "currentColor",
                                            stroke_width: "1.6",
                                            stroke_linecap: "round",
                                        }
                                    }
                                }
                            }
                            div { class: "flex items-start flex-col gap-0.5 cursor-default",
                                span { class: "text-xs", "{user_name}" }
                                span { class: "text-1xs text-foreground/50 truncate max-w-[200px]",
                                    "{user_email}"
                                }
                            }
                        }
                        DropdownMenuContent {
                            side: Some(DropdownSide::Top),
                            align: Some(DropdownAlign::Start),
                            id: use_signal(|| None::<String>),
                            class: "min-w-44",
                            div { class: "flex items-start flex-col gap-1 p-2 cursor-default",
                                span { class: "text-xs", "{user_name}" }
                                span { class: "text-1xs text-foreground/50", "{user_email}" }
                            }
                            DropdownMenuSeparator {}
                            DropdownMenuItem::<String> {
                                value: use_signal(|| "logout".to_string()),
                                index: use_signal(|| 1),
                                disabled: use_signal(|| false),
                                on_select: move |_| {
                                    profile_menu_open.set(Some(false));
                                    logout.call(());
                                },
                                destructive: true,
                                "Logout"
                            }
                        }
                    }
                }
            }
        }
    };

    rsx! {
        // Content row
        div { class: "flex flex-1 min-h-0 relative",
            div { class: "flex flex-1 min-h-0 relative",
                {sidebar}

                // Main content column: scrollable content + bottom bar with org/project
                div { class: "flex-1 min-h-0 flex flex-col",
                    // Main content (style background via the class below, e.g. bg-background or bg-white)
                    div { class: "flex-1 min-h-0 overflow-y-auto py-4.5 px-4.5 bg-content-background",
                        Outlet::<Route> {}
                    }

                    // Bottom bar: org and project selectors (to the right of the sidebar, on every sidebar view)
                    if auth_state.get().is_ok()
                        && (state.selected_context().is_some() || !orgs_snapshot.is_empty())
                    {
                        div { class: "shrink-0 flex items-center gap-2 px-4 py-2 border-t border-app-border bg-background",
                            div {
                                class: "min-w-0",
                                style: "width: min(max-content, clamp(15ch, 12vw, 22ch));",
                                Select {
                                    value: selected_org_id.read().clone(),
                                    on_value_change: move |value: Option<String>| {
                                        let Some(value) = value else { return };
                                        if selected_org_id.read().as_deref() == Some(&value) {
                                            return;
                                        }
                                        selected_org_id.set(Some(value));
                                        selected_project_id.set(None);
                                        pending_org_switch.set(true);
                                    },
                                    placeholder: "Select org".to_string(),
                                    disabled: false,
                                    SelectTrigger { size: SelectSize::Small,
                                        SelectValue { class: "truncate max-w-[250px]" }
                                    }
                                    SelectList {
                                        size: SelectSize::Small,
                                        side: Some(SelectSide::Top),
                                        align: Some(SelectAlign::Start),
                                        if org_options.is_empty() {
                                            SelectOptionItem {
                                                value: "".to_string(),
                                                text_value: "No results".to_string(),
                                                index: 0,
                                                disabled: true,
                                                "No results"
                                            }
                                        } else {
                                            for (i , (id , label)) in org_options.clone().into_iter().enumerate() {
                                                SelectOptionItem {
                                                    value: id.clone(),
                                                    text_value: label.clone(),
                                                    index: i,
                                                    "{label}"
                                                    SelectItemIndicator {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            span { class: "text-foreground/10 text-md", "/" }
                            div {
                                class: "min-w-0",
                                style: "width: min(max-content, clamp(15ch, 12vw, 22ch));",
                                Select {
                                    value: selected_project_id.read().clone(),
                                    on_value_change: move |value: Option<String>| {
                                        let Some(value) = value else { return };
                                        let org_id = match selected_org_id.read().clone() {
                                            Some(id) => id,
                                            None => return,
                                        };
                                        let orgs_snapshot = orgs.read().clone();
                                        let org = orgs_snapshot.iter().find(|org| org.org.resource_id == org_id);
                                        let project = org
                                            .and_then(|o| o.projects.iter().find(|p| p.resource_id == value));
                                        if let (Some(org), Some(project)) = (org, project) {
                                            let ctx = lib::SelectedContext {
                                                org_id: org.org.resource_id.clone(),
                                                org_name: org.org.display_name.clone(),
                                                project_id: project.resource_id.clone(),
                                                project_name: project.display_name.clone(),
                                            };
                                            pending_org_switch.set(false);
                                            spawn({
                                                let state = state.clone();
                                                async move {
                                                    let _ = state.set_selected_context(Some(ctx)).await;
                                                }
                                            });
                                        }
                                        selected_project_id.set(Some(value));
                                    },
                                    placeholder: "Select project".to_string(),
                                    disabled: selected_org_id.read().is_none(),
                                    SelectTrigger { size: SelectSize::Small, SelectValue {} }
                                    SelectList {
                                        size: SelectSize::Small,
                                        align: Some(SelectAlign::End),
                                        side: Some(SelectSide::Top),
                                        if project_options.is_empty() {
                                            SelectOptionItem {
                                                value: "".to_string(),
                                                text_value: "No results".to_string(),
                                                index: 0,
                                                disabled: true,
                                                "No results"
                                            }
                                        } else {
                                            for (i , (id , label)) in project_options.clone().into_iter().enumerate() {
                                                SelectOptionItem {
                                                    value: id.clone(),
                                                    text_value: label.clone(),
                                                    index: i,
                                                    "{label}"
                                                    SelectItemIndicator {}
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            AddTunnelDialog {
                open: add_tunnel_dialog_open(),
                on_open_change: move |open| {
                    add_tunnel_dialog_open.set(open);
                    if !open {
                        editing_tunnel.set(None);
                    }
                },
                initial_proxy: editing_proxy,
                initial_tunnel: Some(editing_tunnel),
                on_save_success: move |_| {
                    editing_tunnel.set(None);
                },
            }
            InviteUserDialog {
                open: invite_user_dialog_open(),
                on_open_change: move |open| invite_user_dialog_open.set(open),
            }
        }
    }
}