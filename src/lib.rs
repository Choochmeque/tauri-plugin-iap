use tauri::{
    Manager, Runtime,
    plugin::{Builder as PluginBuilder, TauriPlugin},
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

pub(crate) mod commands;
mod error;
#[cfg(desktop)]
pub(crate) mod listeners;
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

/// Plugin-wide settings. Build one with [`Builder`].
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Whether the plugin finishes `StoreKit` transactions on your behalf
    /// (macOS only — see [`Builder::finish_transactions_automatically`]).
    pub finish_transactions_automatically: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            // Matches the behavior of every release before this option existed.
            finish_transactions_automatically: true,
        }
    }
}

/// Builds the plugin with non-default [`Config`].
///
/// ```no_run
/// tauri::Builder::default().plugin(
///     tauri_plugin_iap::Builder::new()
///         .finish_transactions_automatically(false)
///         .build(),
/// );
/// ```
#[derive(Debug, Default)]
pub struct Builder {
    config: Config,
}

impl Builder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Controls whether the plugin calls `Transaction.finish()` for you
    /// (**macOS only**; the other platforms ignore this).
    ///
    /// Defaults to `true`, which finishes a transaction as soon as `StoreKit`
    /// verifies it — before your server has seen the receipt. If that server
    /// call then fails, the customer has been charged and the transaction is
    /// gone from `Transaction.updates`, so the only recovery left is Restore.
    ///
    /// Set it to `false` to keep the transaction open until you call
    /// `acknowledgePurchase` (or `consumePurchase`) yourself, which is the
    /// same shape Google Play requires on Android. Transactions you never
    /// acknowledge are re-delivered through `Transaction.updates` on the next
    /// launch, so a failed verification can be retried instead of lost.
    ///
    /// If you set this, you MUST acknowledge; otherwise `StoreKit` re-delivers
    /// the transaction forever and consumables can never be re-bought.
    #[must_use]
    pub const fn finish_transactions_automatically(mut self, finish: bool) -> Self {
        self.config.finish_transactions_automatically = finish;
        self
    }

    #[must_use]
    pub fn build<R: Runtime>(self) -> TauriPlugin<R> {
        let config = self.config;
        PluginBuilder::new("iap")
            .invoke_handler(tauri::generate_handler![
                commands::initialize,
                commands::get_products,
                commands::purchase,
                commands::restore_purchases,
                commands::acknowledge_purchase,
                commands::consume_purchase,
                commands::get_product_status,
                #[cfg(desktop)]
                listeners::register_listener,
                #[cfg(desktop)]
                listeners::remove_listener,
            ])
            .setup(move |app, api| {
                #[cfg(desktop)]
                listeners::init();
                #[cfg(target_os = "macos")]
                let iap = macos::init(app, &api, config)?;
                #[cfg(mobile)]
                let iap = mobile::init(app, &api)?;
                #[cfg(target_os = "windows")]
                let iap = windows::init(app, &api)?;
                #[cfg(target_os = "linux")]
                let iap = desktop::init(app, &api)?;
                // `config` is macOS-only today; keep it live for the others so
                // adding a second setting doesn't have to touch this closure.
                #[cfg(not(target_os = "macos"))]
                let _ = config;
                app.manage(iap);
                Ok(())
            })
            .build()
    }
}

/// Initializes the plugin with default [`Config`].
#[must_use]
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new().build()
}
