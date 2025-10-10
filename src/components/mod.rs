//! The components module contains all shared components for our app. Components are the building blocks of dioxus apps.
//! They can be used to defined common UI elements like buttons, forms, and modals. In this template, we define a Hero
//! component  to be used in our app.

mod hero;

mod button;
mod domains;
mod head;
mod splash;
mod typography;

pub use button::{Button, CloseButton};
pub use domains::{Domain, Domains};
pub use head::Head;
pub use splash::Splash;
pub use typography::Subhead;
