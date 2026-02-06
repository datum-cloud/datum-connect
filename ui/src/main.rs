use dioxus::prelude::*;
#[cfg(feature = "desktop")]
use n0_error::Result;
use std::sync::OnceLock;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::components::{Head, Splash};
use crate::state::AppState;
use crate::views::{
    Chrome, JoinProxy, Login, ProxiesList, SelectProject, Settings, Sidebar, Signup,
    TunnelBandwidth,
};

#[cfg(feature = "desktop")]
use dioxus_desktop::{
    trayicon::{
        menu::{Menu, MenuItem, PredefinedMenuItem},
        Icon, TrayIcon, TrayIconBuilder,
    },
    use_tray_menu_event_handler, use_window,
};

mod components;
mod state;
mod util;
mod views;

static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// The Route enum is used to define the structure of internal routes in our app. All route enums need to derive
/// the [`Routable`] trait, which provides the necessary methods for the router to work.
///
/// Each variant represents a different URL pattern that can be matched by the router. If that pattern is matched,
/// the components for that route will be rendered.
#[derive(Debug, Clone, Routable, PartialEq)]
#[rustfmt::skip]
enum Route {
    #[layout(Chrome)]
    #[route("/")]
    Login{},
    #[route("/select")]
    SelectProject{},
    #[route("/signup")]
    Signup{},
    // The layout attribute defines a wrapper for all routes under the layout. Layouts are great for wrapping
    // many routes with a common UI like a navbar.
    #[layout(Sidebar)]
        #[route("/proxies")]
        ProxiesList {},
        #[route("/proxy/edit/:id/bandwidth")]
        TunnelBandwidth { id: String },
        #[route("/proxy/join")]
        JoinProxy {},
        #[route("/settings")]
        Settings {},
}

fn main() {
    init_tracing();
    if let Some(path) = dotenv::dotenv().ok() {
        info!("Loaded environment variables from {}", path.display());
    }

    #[cfg(all(feature = "desktop", target_os = "linux"))]
    gtk::init().unwrap();

    #[cfg(feature = "desktop")]
    let _tray_icon = init_menu_bar().unwrap();

    #[cfg(feature = "desktop")]
    {
        // Use a custom titlebar so we can theme the top chrome (height + color).
        use dioxus_desktop::{
            tao::platform::macos::WindowBuilderExtMacOS, Config, LogicalSize, WindowBuilder,
            WindowCloseBehaviour,
        };

        dioxus::LaunchBuilder::desktop()
            .with_cfg(desktop! {
                Config::new()
                    // Make "close" behave like hide, so the tray icon can restore it.
                    .with_close_behaviour(WindowCloseBehaviour::WindowHides)
                    .with_window(
                        WindowBuilder::new()
                            .with_title("")
                            .with_inner_size(LogicalSize::new(700, 600))  // default width, height (logical pixels)
                            .with_min_inner_size(LogicalSize::new(700, 600))  // prevent resizing smaller
                            .with_resizable(false)
                            .with_window_icon(Some(window_icon()))
                            .with_transparent(true)
                            .with_has_shadow(true)
                    )
            })
            .launch(App);
    }

    #[cfg(not(feature = "desktop"))]
    dioxus::launch(App);
}

fn init_tracing() {
    let repo_path = lib::Repo::default_location();
    if let Err(err) = std::fs::create_dir_all(&repo_path) {
        eprintln!(
            "ui: failed to create repo dir {}: {err}",
            repo_path.display()
        );
    }
    let file_appender = tracing_appender::rolling::never(&repo_path, "ui.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(std::io::stderr))
        .with(fmt::layer().with_writer(non_blocking))
        .init();
}

#[component]
fn App() -> Element {
    let mut app_state_ready = use_signal(|| false);
    use_future(move || async move {
        let state = AppState::load().await.unwrap();
        // Refresh user profile on app startup to fetch latest data from API
        if state.datum().login_state() != lib::datum_cloud::LoginState::Missing {
            if let Err(err) = state.datum().auth().refresh_profile().await {
                tracing::warn!("Failed to refresh user profile on startup: {err:#}");
            }
        }
        // let nav = navigator();
        // if state.datum().login_state() == LoginState::Missing {
        //     nav.push(Route::Login {});
        // }
        provide_context(state);
        app_state_ready.set(true);
    });

    // Set macOS menu bar name, dock icon, window corner radius, and decorations after the app launches (run loop must be active)
    #[cfg(all(feature = "desktop", target_os = "macos"))]
    {
        use_effect(|| {
            set_macos_menu_name();
            // Try to set decorations with multiple retries to ensure window is ready
            // Use exponential backoff: 10ms, 50ms, 100ms, 200ms, 500ms
            spawn(async move {
                for delay_ms in [10, 50, 100, 200, 500] {
                    tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    set_macos_window_decorations();
                }
            });
        });

        // Also apply decorations when app state becomes ready (window should be ready by then)
        use_effect(move || {
            if app_state_ready() {
                spawn(async move {
                    // Small delay to ensure window is fully rendered
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    set_macos_window_decorations();
                    // One more retry after a longer delay
                    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                    set_macos_window_decorations();
                });
            }
        });
    }

    #[cfg(feature = "desktop")]
    use_tray_menu_event_handler(move |event| {
        // The event ID corresponds to the menu item text
        match event.id.0.as_str() {
            "About Datum" => {
                let _ = open::that("https://datum.net");
            }
            "Show Window" => {
                let window = use_window();
                window.set_visible(true);
                // Reapply decorations when window is shown (in case they didn't apply initially)
                #[cfg(target_os = "macos")]
                {
                    spawn(async move {
                        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
                        set_macos_window_decorations();
                    });
                }
            }
            "Hide" => {
                use_window().set_visible(false);
            }
            "Quit" => {
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown menu event: {}", event.id.0);
            }
        }
    });

    if !app_state_ready() {
        return rsx! {
            Head {}
            Splash {}
        };
    }

    // Signal bumped on login/logout so title bar and other auth-dependent UI re-render.
    let mut auth_changed = use_signal(|| 0u32);
    provide_context(auth_changed);

    rsx! {
        Head {}
        Router::<Route> {}
    }
}

#[cfg(feature = "desktop")]
fn init_menu_bar() -> Result<TrayIcon> {
    // Initialize the tray menu

    use n0_error::StdResultExt;
    let tray_menu = Menu::new();

    // Create menu items with IDs for event handling
    let about_item = MenuItem::new("About Datum", true, None);
    let show_item = MenuItem::new("Show Window", true, None);
    let hide_item = MenuItem::new("Hide", true, None);
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::new("Quit", true, None);

    // Build the menu structure (macOS-style: About, Show, Hide, sep, Quit)
    tray_menu
        .append_items(&[&about_item, &show_item, &hide_item, &separator, &quit_item])
        .expect("Failed to build tray menu");

    let icon = icon();

    // Build the tray icon
    TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Datum")
        .with_icon(icon)
        .build()
        .std_context("building tray icon")
}

/// Load an icon from a PNG file for the tray
#[cfg(feature = "desktop")]
fn icon() -> Icon {
    use image::GenericImageView;

    let icon_bytes = include_bytes!("../assets/bundle/linux/512.png");
    let image = image::load_from_memory(icon_bytes).unwrap();

    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8().into_raw();

    Icon::from_rgba(rgba, width, height).expect("Failed to create icon from image")
}

/// Load an icon from a PNG file for the window
#[cfg(feature = "desktop")]
fn window_icon() -> dioxus_desktop::tao::window::Icon {
    use image::GenericImageView;

    let icon_bytes = include_bytes!("../assets/bundle/linux/512.png");
    let image = image::load_from_memory(icon_bytes).unwrap();

    let (width, height) = image.dimensions();
    let rgba = image.to_rgba8().into_raw();

    dioxus_desktop::tao::window::Icon::from_rgba(rgba, width, height)
        .expect("Failed to create window icon from image")
}

/// Custom Objective-C class to handle menu actions
#[cfg(all(feature = "desktop", target_os = "macos"))]
mod macos_menu_handler {
    use objc2::rc::Retained;
    use objc2::runtime::NSObject;
    use objc2::{define_class, extern_methods};
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSObject as FoundationNSObject, NSString, NSURL};

    define_class!(
        #[unsafe(super(FoundationNSObject))]
        pub struct MenuActionHandler;

        impl MenuActionHandler {
            #[unsafe(method(openAboutURL:))]
            fn open_about_url(&self, _sender: Option<&NSObject>) {
                // Open https://datum.net in the default browser
                let url_str = NSString::from_str("https://datum.net");
                if let Some(url) = NSURL::URLWithString(&url_str) {
                    // SAFETY: sharedWorkspace is safe to call
                    unsafe {
                        let workspace = NSWorkspace::sharedWorkspace();
                        let _ = workspace.openURL(&url);
                    }
                }
            }

            #[unsafe(method(checkForUpdates:))]
            fn check_for_updates(&self, _sender: Option<&NSObject>) {
                // Placeholder for check for updates functionality
                // TODO: Implement update checking
            }
        }
    );

    // Expose the `new` method from NSObject
    impl MenuActionHandler {
        extern_methods!(
            #[unsafe(method(new))]
            pub fn new() -> Retained<Self>;
        );
    }
}

/// Set the macOS menu bar app name and add custom menu items.
#[cfg(all(feature = "desktop", target_os = "macos"))]
fn set_macos_menu_name() {
    use objc2::runtime::Sel;
    use objc2::{ClassType, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSMenuItem};
    use objc2_foundation::NSString;
    use std::ffi::CStr;
    use std::sync::Once;

    // SAFETY: We're on the main thread (called from use_effect in the UI)
    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    let app = NSApplication::sharedApplication(mtm);
    let app_name = NSString::from_str("Datum");

    // Set the menu bar app name by modifying the main menu's first item (app menu)
    if let Some(main_menu) = app.mainMenu() {
        if let Some(app_menu_item) = main_menu.itemAtIndex(0) {
            app_menu_item.setTitle(&app_name);

            // Also update the submenu title and add custom items
            if let Some(app_submenu) = app_menu_item.submenu() {
                app_submenu.setTitle(&app_name);

                static MENU_ITEMS_ADDED: Once = Once::new();
                static HANDLER: std::sync::OnceLock<
                    objc2::rc::Retained<macos_menu_handler::MenuActionHandler>,
                > = std::sync::OnceLock::new();
                MENU_ITEMS_ADDED.call_once(|| {
                    // Register the custom handler class
                    macos_menu_handler::MenuActionHandler::class();

                    // Create an instance of the handler to use as target (store it statically so it's retained)
                    let handler = macos_menu_handler::MenuActionHandler::new();
                    HANDLER.set(handler.clone()).ok();

                    let empty_key = NSString::from_str("");

                    // Add "About Datum" at the beginning
                    let about_title = NSString::from_str("About Datum");
                    unsafe {
                        let about_sel =
                            Sel::register(CStr::from_bytes_with_nul(b"openAboutURL:\0").unwrap());
                        let about_item = app_submenu
                            .insertItemWithTitle_action_keyEquivalent_atIndex(
                                &about_title,
                                Some(about_sel),
                                &empty_key,
                                0,
                            );
                        about_item.setTarget(Some(&*handler));
                        about_item.setImage(None);
                        about_item.setOnStateImage(None);
                        about_item.setOffStateImage(None);
                    }

                    // Add separator after "About Datum"
                    unsafe {
                        let separator = NSMenuItem::separatorItem(mtm);
                        app_submenu.insertItem_atIndex(&separator, 1);
                    }

                    // Add "Check for Updates..."
                    let updates_title = NSString::from_str("Check for Updates...");
                    unsafe {
                        let updates_sel = Sel::register(
                            CStr::from_bytes_with_nul(b"checkForUpdates:\0").unwrap(),
                        );
                        let updates_item = app_submenu
                            .insertItemWithTitle_action_keyEquivalent_atIndex(
                                &updates_title,
                                Some(updates_sel),
                                &empty_key,
                                2,
                            );
                        updates_item.setTarget(Some(&*handler));
                        updates_item.setImage(None);
                        updates_item.setOnStateImage(None);
                        updates_item.setOffStateImage(None);
                    }
                });
            }
        }
    }
}

/// Customize macOS window decorations/titlebar appearance.
/// This function is designed to be called multiple times safely - it will apply settings
/// even if called repeatedly.
#[cfg(all(feature = "desktop", target_os = "macos"))]
fn set_macos_window_decorations() {
    use objc2::runtime::AnyClass;
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    use objc2_foundation::NSString;
    use std::ffi::CStr;

    // SAFETY: We're on the main thread (called from use_effect in the UI)
    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    // Get the main window through NSApplication
    let app = NSApplication::sharedApplication(mtm);
    unsafe {
        let app_ref: &NSApplication = app.as_ref();
        let ns_window: Option<objc2::rc::Retained<objc2::runtime::NSObject>> =
            objc2::msg_send![app_ref, mainWindow];

        if let Some(ns_window) = ns_window {
            let ns_window_ref: &objc2::runtime::NSObject = ns_window.as_ref();

            // Check if window is ready by verifying it has a content view
            // This helps ensure the window is fully initialized
            let content_view: Option<objc2::rc::Retained<objc2::runtime::NSObject>> =
                objc2::msg_send![ns_window_ref, contentView];

            if content_view.is_none() {
                // Window not ready yet, will be retried
                return;
            }

            let content_view = content_view.unwrap();
            let content_view_ref: &objc2::runtime::NSObject = content_view.as_ref();

            // Make titlebar transparent FIRST so we can customize it
            let _: () = objc2::msg_send![ns_window_ref, setTitlebarAppearsTransparent: true];

            // Set background color on the contentView instead of the window
            // This is more reliable as it directly affects what's visible
            // Using #efefed - glacier-mist-800
            let color_class_name = CStr::from_bytes_with_nul(b"NSColor\0").unwrap();
            let color_class = AnyClass::get(color_class_name).expect("NSColor class should exist");
            // #efefed converts to sRGB(239/255, 239/255, 237/255)
            let custom_color: objc2::rc::Retained<objc2::runtime::NSObject> = objc2::msg_send![color_class, colorWithSRGBRed: 239.0/255.0, green: 239.0/255.0, blue: 237.0/255.0, alpha: 1.0f64];
            let custom_color_ref: &objc2::runtime::NSObject = custom_color.as_ref();

            // Set background color on contentView (more reliable than window)
            let _: () = objc2::msg_send![content_view_ref, setWantsLayer: true];
            let _: () = objc2::msg_send![content_view_ref, setBackgroundColor: custom_color_ref];

            // Also set on window as fallback
            let _: () = objc2::msg_send![ns_window_ref, setBackgroundColor: custom_color_ref];

            // Hide the title (since we have custom controls)
            // NSWindowTitleHidden = 1
            let _: () = objc2::msg_send![ns_window_ref, setTitleVisibility: 1u64];

            // Set the window's appearance AFTER background color and transparency
            // Use light appearance (NSAppearanceNameAqua) since our background is light
            // This ensures the window controls and titlebar elements render correctly
            let appearance_class_name = CStr::from_bytes_with_nul(b"NSAppearance\0").unwrap();
            let appearance_class =
                AnyClass::get(appearance_class_name).expect("NSAppearance class should exist");
            let appearance_name = NSString::from_str("NSAppearanceNameAqua");
            let appearance_name_ref: &NSString = appearance_name.as_ref();
            let appearance: objc2::rc::Retained<objc2::runtime::NSObject> =
                objc2::msg_send![appearance_class, appearanceNamed: appearance_name_ref];
            let appearance_ref: &objc2::runtime::NSObject = appearance.as_ref();
            let _: () = objc2::msg_send![ns_window_ref, setAppearance: appearance_ref];

            // Force the window and content view to update their appearance and redraw
            let _: () = objc2::msg_send![content_view_ref, setNeedsDisplay: true];
            let _: () = objc2::msg_send![ns_window_ref, invalidateShadow];
            let _: () = objc2::msg_send![ns_window_ref, display];
        }
    }
}
