// This file is adapted from async_std

#[macro_export]
macro_rules! task_local {
    () => ();

    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr) => (
        $(#[$attr])* $vis static $name: crate::task::LocalKey<$t> = {
            #[inline]
            fn __init() -> $t {
                $init
            }

            crate::task::LocalKey::new(__init)
        };
    );

    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; $($rest:tt)*) => (
        self::task_local!($(#[$attr])* $vis static $name: $t = $init);
        self::task_local!($($rest)*);
    );
}
