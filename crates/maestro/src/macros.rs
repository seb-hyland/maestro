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
    ( $( $expr:expr ),* $(,)? ) => {
        [
            $(
                ::std::path::Path::new($expr)
            ),*
        ]
    };
}

#[macro_export]
macro_rules! assert_exists {
    ( $( $expr:expr ),* $(,)? ) => {{
        let mut failing_paths = Vec::new();
        $(
            $crate::OutputChecker::check_path(&$expr, &mut failing_paths);
        )*
        if !failing_paths.is_empty() {
            let display_paths: Vec<_> = failing_paths
                .into_iter()
                .map(|p| p.display().to_string())
                .collect();
            Err(
                ::std::io::Error::new(
                    ::std::io::ErrorKind::NotFound, format!(
                        "The paths [{}] were expected to exist, but do not!", display_paths.join(",")
                    )
                )
            )
        } else {
            Ok(())
        }
    }?};
}
