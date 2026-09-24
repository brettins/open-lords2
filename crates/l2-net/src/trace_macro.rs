//! Emit macros: without `--features trace` each expands to nothing, arguments unevaluated. CNEW-trace-sink.

#[cfg(feature = "trace")]
#[macro_export]
macro_rules! trace {
    ($kind:literal, $addr:literal, $name:literal $(, $k:ident = $v:expr)* $(,)?) => {
        if $crate::trace::on() {
            $crate::trace::emit($kind, $addr, $name,
                vec![$((stringify!($k), $crate::trace::IntoArg::into_arg($v))),*]);
        }
    };
}

#[cfg(not(feature = "trace"))]
#[macro_export]
macro_rules! trace {
    ($($t:tt)*) => {};
}

/// `trace_changed!(value; kind, addr, name, k = v…)`: emits only when `value` differs from the last.
#[cfg(feature = "trace")]
#[macro_export]
macro_rules! trace_changed {
    ($value:expr; $kind:literal, $addr:literal, $name:literal $(, $k:ident = $v:expr)* $(,)?) => {
        if $crate::trace::changed($name, ($value) as i64) {
            $crate::trace::emit($kind, $addr, $name,
                vec![$((stringify!($k), $crate::trace::IntoArg::into_arg($v))),*]);
        }
    };
}

#[cfg(not(feature = "trace"))]
#[macro_export]
macro_rules! trace_changed {
    ($($t:tt)*) => {};
}

/// Statements that exist only in a traced build.
#[cfg(feature = "trace")]
#[macro_export]
macro_rules! trace_do {
    ($($t:tt)*) => { $($t)* };
}

#[cfg(not(feature = "trace"))]
#[macro_export]
macro_rules! trace_do {
    ($($t:tt)*) => {};
}
