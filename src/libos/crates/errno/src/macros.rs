#[macro_export]
macro_rules! errno {
    ($errno_expr: expr, $error_msg: expr) => {{
        let inner_error = {
            let errno: Errno = $errno_expr;
            let msg: &'static str = $error_msg;
            (errno, msg)
        };
        let error =
            $crate::Error::embedded(inner_error, Some(ErrorLocation::new(file!(), line!())));
        error
    }};
    ($error_expr: expr) => {{
        let inner_error = $error_expr;
        let error = $crate::Error::boxed(inner_error, Some(ErrorLocation::new(file!(), line!())));
        error
    }};
}

#[macro_export]
macro_rules! return_errno {
    ($errno_expr: expr, $error_msg: expr) => {{
        return Err(errno!($errno_expr, $error_msg));
    }};
    ($error_expr: expr) => {{
        return Err(errno!($error_expr));
    }};
}
