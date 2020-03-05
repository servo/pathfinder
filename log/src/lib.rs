pub extern crate log;

#[macro_export]
macro_rules! pa_log {
    ($($t:tt)*) => (
        #[cfg(feature="log_warn")]
        $crate::log::log!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_debug {
    ($($t:tt)*) => (
        #[cfg(feature="log_debug")]
        $crate::log::debug!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_trace {
    ($($t:tt)*) => (
        #[cfg(feature="log_trace")]
        $crate::log::trace!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_info {
    ($($t:tt)*) => (
        #[cfg(feature="log_info")]
        $crate::log::info!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_warn {
    ($($t:tt)*) => (
        #[cfg(feature="log_warn")]
        $crate::log::warn!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_error {
    ($($t:tt)*) => (
        $crate::log::error!($($t)*)
    )
}
