pub extern crate log;

#[macro_export]
macro_rules! pa_log {
    ($($t:tt)*) => (
        $crate::log::log!($($t)*)
    )
}

#[macro_export]
#[cfg(feature="log_debug")]
macro_rules! pa_debug {
    ($($t:tt)*) => (
        $crate::log::debug!($($t)*)
    )
}
#[macro_export]
#[cfg(not(feature="log_debug"))]
macro_rules! pa_debug {
    ($($t:tt)*) => ()
}

#[macro_export]
#[cfg(feature="log_trace")]
macro_rules! pa_trace {
    ($($t:tt)*) => (
        $crate::log::trace!($($t)*)
    )
}
#[macro_export]
#[cfg(not(feature="log_trace"))]
macro_rules! pa_trace {
    ($($t:tt)*) => ()
}

#[macro_export]
#[cfg(feature="log_info")]
macro_rules! pa_info {
    ($($t:tt)*) => (
        $crate::log::info!($($t)*)
    )
}
#[macro_export]
#[cfg(not(feature="log_info"))]
macro_rules! pa_info {
    ($($t:tt)*) => ()
}

#[macro_export]
#[cfg(feature="log_warn")]
macro_rules! pa_warn {
    ($($t:tt)*) => (
        $crate::log::warn!($($t)*)
    )
}
#[macro_export]
#[cfg(not(feature="log_warn"))]
macro_rules! pa_warn {
    ($($t:tt)*) => ()
}

#[macro_export]
macro_rules! pa_error {
    ($($t:tt)*) => (
        $crate::log::error!($($t)*)
    )
}
