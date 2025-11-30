use tauri::{
    plugin::{Builder, TauriPlugin},
    Manager, Runtime,
};

pub use models::*;

#[cfg(target_os = "linux")]
mod desktop;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(mobile)]
mod mobile;
#[cfg(target_os = "windows")]
mod windows;

mod commands;
mod error;
mod models;

pub use error::{Error, Result};

#[cfg(target_os = "linux")]
use desktop::Iap;
#[cfg(target_os = "macos")]
use macos::Iap;
#[cfg(mobile)]
use mobile::Iap;
#[cfg(target_os = "windows")]
use windows::Iap;

/// Extensions to [`tauri::App`], [`tauri::AppHandle`] and [`tauri::Window`] to access the iap APIs.
pub trait IapExt<R: Runtime> {
    fn iap(&self) -> &Iap<R>;
}

impl<R: Runtime, T: Manager<R>> crate::IapExt<R> for T {
    fn iap(&self) -> &Iap<R> {
        self.state::<Iap<R>>().inner()
    }
}

/// Initializes the plugin.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("iap")
        .invoke_handler(tauri::generate_handler![
            commands::initialize,
            commands::get_products,
            commands::purchase,
            commands::restore_purchases,
            commands::acknowledge_purchase,
            commands::get_product_status,
        ])
        .setup(|app, api| {
            #[cfg(target_os = "macos")]
            let iap = macos::init(app, api)?;
            #[cfg(mobile)]
            let iap = mobile::init(app, api)?;
            #[cfg(target_os = "windows")]
            let iap = windows::init(app, api)?;
            #[cfg(target_os = "linux")]
            let iap = desktop::init(app, api)?;
            app.manage(iap);
            Ok(())
        })
        .build()
}
