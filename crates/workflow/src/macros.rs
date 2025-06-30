/// Example usage:
/// ```rust
/// let path_b = PathBuf::from("b.txt");
/// paths![
///     "a.txt",
///     path_b
/// ]
/// ```
#[macro_export]
macro_rules! paths {
    ( $( $token:tt ),* $(,)? ) => {{
        vec![
            $(
                paths!(@parse $token)
            ),*
        ]
    }};

    (@parse $lit:literal) => {
        ::std::path::PathBuf::from($lit)
    };
    (@parse $id:ident) => {
        $id
    };
}
