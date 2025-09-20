use maestro::prelude::*;

fn main() {
    test_workflow(0).unwrap();
}

fn test_workflow(runid: i32) -> io::Result<Vec<PathBuf>> {
    let input_path = Path::new("data/greeting.txt");
    let out_path = Path::new("out.txt");

    let process = process! {
        name = format!("test_workflow_{runid}"),
        inputs = [input_path],
        outputs = [out_path],
        process = r#"
        cat "$input_path" > "$out_path"
        "#
    };
    MAESTRO_CONFIG.exe(process)
}
