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
        [
            $(
                Path::new($token)
            ),*
        ]
    }};
}
/// Example usage:
/// ```rust
/// let path_a = PathBuf::from("a.txt");
/// let path_b = PathBuf::from("b.txt");
/// let my_workflows = workflows! {
///     MyWorkflow(path_a),
///     MyWorkflow(path_b)
/// };
/// ...
/// #[workflow]
/// fn MyWorkflow(input: PathBuf) -> Workflow {
///     ...
/// ```
#[macro_export]
macro_rules! workflows {
    ( $( $workflow:expr ),* $(,)? ) => {{
        let workflows: Vec<Workflow> = vec![
            $(
                $workflow
            ),*
        ];
        workflows
    }};
}
