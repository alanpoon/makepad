pub use makepad_widgets;

// The demo GUI (and its `app_main!`, which defines the Android JNI entry points)
// is only needed by this crate's own binary. Gate it behind a default feature so
// library consumers that just want the pipeline can turn it off with
// `default-features = false` and avoid duplicate-symbol link errors on Android.
#[cfg(feature = "app")]
pub mod app;
pub mod hand;
pub mod image_io;
pub mod overlay;
pub mod palm;
pub mod pipeline;
pub mod roi;
