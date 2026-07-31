//! Device motion-sensor events, delivered after `Cx::start_gyroscope_updates()`.
//!
//! Backends: Android `SensorManager` (`Sensor.TYPE_GYROSCOPE`). Other
//! platforms have no motion backend and stay silent.

/// How often the platform should deliver motion samples. This is a hint: the
/// OS batches and rate-limits as it sees fit, and the actual interval between
/// samples must be read from the sample timestamps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionUpdateRate {
    /// Slowest rate, suitable for screen-orientation style updates
    /// (Android `SENSOR_DELAY_NORMAL`, ~5Hz).
    Normal,
    /// Rate suitable for driving UI (Android `SENSOR_DELAY_UI`, ~16Hz).
    Ui,
    /// Rate suitable for games and camera control
    /// (Android `SENSOR_DELAY_GAME`, ~50Hz).
    Game,
    /// As fast as the sensor delivers (Android `SENSOR_DELAY_FASTEST`).
    /// Expect a high event volume and matching battery cost.
    Fastest,
}

impl MotionUpdateRate {
    /// Wire code passed to the platform layer.
    pub fn to_code(&self) -> i32 {
        match self {
            Self::Normal => 0,
            Self::Ui => 1,
            Self::Game => 2,
            Self::Fastest => 3,
        }
    }
}

/// One gyroscope sample: angular rate around the three device axes in
/// radians per second, right-handed, relative to the device's natural
/// orientation (x = right along the short edge, y = up along the long edge,
/// z = out of the screen towards the user).
#[derive(Clone, Debug, PartialEq)]
pub struct GyroscopeUpdateEvent {
    /// Rotation rate around the x axis (device tilting away/towards you).
    pub rate_x: f64,
    /// Rotation rate around the y axis (device tilting left/right).
    pub rate_y: f64,
    /// Rotation rate around the z axis (device spinning flat).
    pub rate_z: f64,
    /// Sample time in seconds on a monotonic clock — boot-relative on
    /// Android, so only differences between samples are meaningful. Use
    /// those differences to integrate rates into an angle.
    pub time: f64,
}

/// Motion updates cannot be delivered (once per start attempt). This is
/// terminal for that start attempt: updates do not resume on their own.
#[derive(Clone, Debug, PartialEq)]
pub enum MotionErrorEvent {
    /// No such sensor on this device, no motion backend on this platform,
    /// or an OS error while subscribing.
    Unavailable(String),
}
