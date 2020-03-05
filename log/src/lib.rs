#[cfg(feature="log_warn")]
#[macro_use]
extern crate log;

#[macro_export]
macro_rules! pa_log {
    ($(t:tt)*) => (
        #[cfg(feature="log_warn")]
        log!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_debug {
    ($(t:tt)*) => (
        #[cfg(feature="log_debug")]
        debug!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_trace {
    ($(t:tt)*) => (
        #[cfg(feature="log_trace")]
        trace!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_info {
    ($(t:tt)*) => (
        #[cfg(feature="log_info")]
        info!($($t)*)
    )
}

#[macro_export]
macro_rules! pa_warn {
    ($(t:tt)*) => (
        #[cfg(feature="log_warn")]
        warn!($($t)*)
    )
}

