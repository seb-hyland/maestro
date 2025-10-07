use maestro::prelude::*;

#[maestro::main]
fn main() {
    println!("{}", arg!("init_msg"));
    test_workflow(0).unwrap();
}

fn test_workflow(runid: i32) -> WorkflowResult {
    let input_path = Path::new("data/greeting.txt");
    let out_path = Path::new("out.txt");

    process! {
        name = format!("test_workflow_{runid}"),
        executor = "default",
        inputs = [input_path],
        outputs = [out_path],
        script = r#"
        cat "$input_path" > "$out_path"
        "#
    }
}
