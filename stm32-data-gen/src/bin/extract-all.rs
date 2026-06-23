use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let peri = std::env::args().nth(1).expect("missing peripheral name");

    let tmp_dir = format!("tmp/{}", peri);
    let svd_dir = Path::new("sources/svd");

    // rm -rf tmp/$peri
    let _ = fs::remove_dir_all(&tmp_dir);

    // mkdir -p tmp/$peri
    fs::create_dir_all(&tmp_dir).unwrap();

    // iterate over SVD files
    for entry in fs::read_dir(svd_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) != Some("svd") {
            continue;
        }

        let filename = path.file_name().unwrap().to_string_lossy();

        // strip "stm32" prefix and ".svd" suffix
        let mut f = filename.to_string();
        if let Some(stripped) = f.strip_prefix("stm32") {
            f = stripped.to_string();
        }
        if let Some(stripped) = f.strip_suffix(".svd") {
            f = stripped.to_string();
        }

        print!("processing {} ... ", f);

        let yaml_path = format!("{}/{}.yaml", tmp_dir, f);
        let err_path = format!("{}/{}.err", tmp_dir, f);

        // run chiptool
        let output = Command::new("chiptool")
            .arg("extract-peripheral")
            .arg("--svd")
            .arg(path.to_str().unwrap())
            .arg("--peripheral")
            .arg(&peri)
            .args(std::env::args().skip(2)) // pass-through extra args
            .output()
            .expect("failed to run chiptool");

        // write stdout and stderr
        fs::write(&yaml_path, &output.stdout).unwrap();
        fs::write(&err_path, &output.stderr).unwrap();

        if output.status.success() {
            // OK
            let _ = fs::remove_file(&err_path);
            println!("OK");
        } else {
            // check error type
            let err_text = String::from_utf8_lossy(&output.stderr);
            if err_text.contains("peripheral not found") {
                println!("No Peripheral");
            } else {
                println!("OTHER FAILURE");
            }
            let _ = fs::remove_file(&yaml_path);
        }
    }
}
