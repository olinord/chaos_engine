macro_rules! impl_swizzle2_for_first {
    ($return_type:ident, $a:ident; $($b:ident),+ $(,)?) => {
        paste::paste! {
            $(
                pub fn [<$a $b>](&self) -> $return_type {
                    $return_type {
                        x: self.$a,
                        y: self.$b,
                    }
                }
            )+
        }
    };
}

macro_rules! impl_swizzle2_all {
    (($($all:ident),+), $return_type:ident;) => {};
    (($($all:ident),+), $return_type:ident; $a:ident $(, $rest:ident)* $(,)?) => {
        impl_swizzle2_for_first!($return_type, $a; $($all),+);
        impl_swizzle2_all!(($($all),+), $return_type; $($rest),*);
    };
}

macro_rules! impl_swizzle3_for_prefix {
    ($return_type:ident, $a:ident, $b:ident; $($c:ident),+ $(,)?) => {
        paste::paste! {
            $(
                pub fn [<$a $b $c>](&self) -> $return_type {
                    $return_type {
                        x: self.$a,
                        y: self.$b,
                        z: self.$c,
                    }
                }
            )+
        }
    };
}

macro_rules! impl_swizzle3_for_first {
    (($($all:ident),+), $return_type:ident, $a:ident;) => {};
    (($($all:ident),+), $return_type:ident, $a:ident; $b:ident $(, $rest:ident)* $(,)?) => {
        impl_swizzle3_for_prefix!($return_type, $a, $b; $($all),+);
        impl_swizzle3_for_first!(($($all),+), $return_type, $a; $($rest),*);
    };
}

macro_rules! impl_swizzle3_all {
    (($($all:ident),+), $return_type:ident;) => {};
    (($($all:ident),+), $return_type:ident; $a:ident $(, $rest:ident)* $(,)?) => {
        impl_swizzle3_for_first!(($($all),+), $return_type, $a; $($all),+);
        impl_swizzle3_all!(($($all),+), $return_type; $($rest),*);
    };
}

macro_rules! impl_swizzle4_for_prefix {
    ($return_type:ident, $a:ident, $b:ident, $c:ident; $($d:ident),+ $(,)?) => {
        paste::paste! {
            $(
                pub fn [<$a $b $c $d>](&self) -> $return_type {
                    $return_type {
                        x: self.$a,
                        y: self.$b,
                        z: self.$c,
                        w: self.$d,
                    }
                }
            )+
        }
    };
}

macro_rules! impl_swizzle4_for_pair {
    (($($all:ident),+), $return_type:ident, $a:ident, $b:ident;) => {};
    (($($all:ident),+), $return_type:ident, $a:ident, $b:ident; $c:ident $(, $rest:ident)* $(,)?) => {
        impl_swizzle4_for_prefix!($return_type, $a, $b, $c; $($all),+);
        impl_swizzle4_for_pair!(($($all),+), $return_type, $a, $b; $($rest),*);
    };
}

macro_rules! impl_swizzle4_for_first {
    (($($all:ident),+), $return_type:ident, $a:ident;) => {};
    (($($all:ident),+), $return_type:ident, $a:ident; $b:ident $(, $rest:ident)* $(,)?) => {
        impl_swizzle4_for_pair!(($($all),+), $return_type, $a, $b; $($all),+);
        impl_swizzle4_for_first!(($($all),+), $return_type, $a; $($rest),*);
    };
}

macro_rules! impl_swizzle4_all {
    (($($all:ident),+), $return_type:ident;) => {};
    (($($all:ident),+), $return_type:ident; $a:ident $(, $rest:ident)* $(,)?) => {
        impl_swizzle4_for_first!(($($all),+), $return_type, $a; $($all),+);
        impl_swizzle4_all!(($($all),+), $return_type; $($rest),*);
    };
}
