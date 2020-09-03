// Macros to implement `PrctlCmd` given a list of prctl names, numbers, and argument types.

// Implement `PrctlNum` and `PrctlCmd`.
#[macro_export]
macro_rules! impl_prctl_nums_and_cmds {
    ($( $prctl_name: ident => ( $prctl_num: expr, $($prctl_type_tt: tt),* ) ),+,) => {
        $(const $prctl_name:i32 = $prctl_num;)*

        impl_prctl_cmds! {
            $(
                $prctl_name => ( $($prctl_type_tt),* ),
            )*
        }
    }
}

macro_rules! impl_prctl_cmds {
    ($( $prctl_name: ident => ( $($prctl_type_tt: tt),* ) ),+,) => {
        #[derive(Debug)]
        #[allow(non_camel_case_types)]
        pub enum PrctlCmd<'a> {
            $(
                $prctl_name($(get_arg_type!($prctl_type_tt)),* ),
            )*
        }
    }
}

macro_rules! get_arg_type {
    ($prctl_type_tt: tt) => {
        $prctl_type_tt
    };
}
