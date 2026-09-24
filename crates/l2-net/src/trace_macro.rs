//! Emit macros: without `--features trace` each expands to nothing, arguments unevaluated. C246.

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

/// `trace_leaf!("kind", 0x........, "Name", k = v…; body)`: a hooked leaf's record around `body`,
/// whose own draws and text are not recorded (FORMAT.md, last line). Address 0 holds, no record.
#[cfg(feature = "trace")]
#[macro_export]
macro_rules! trace_leaf {
    ($kind:literal, $addr:literal, $name:literal $(, $k:ident = $v:expr)*; $body:expr) => {{
        $crate::trace_draw::enter($kind, $addr, $name,
            vec![$((stringify!($k), $crate::trace::IntoArg::into_arg($v))),*]);
        let r = $body;
        $crate::trace_draw::leave();
        r
    }};
}

#[cfg(not(feature = "trace"))]
#[macro_export]
macro_rules! trace_leaf {
    ($kind:literal, $addr:literal, $name:literal $(, $k:ident = $v:expr)*; $body:expr) => {
        $body
    };
}