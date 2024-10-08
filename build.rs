use std::{env, fs::File, io::Write, path::PathBuf, process::Command};

fn main() {
    // Generate third-party license file when the lockfile changes
    println!("cargo::rerun-if-changed=Cargo.lock");
    println!("cargo::rerun-if-changed=about.hbs");
    let license_html = Command::new("cargo")
        .arg("about")
        .arg("generate")
        .arg(
            PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("no manifest dir env var"))
                .join("assets/about.hbs"),
        )
        .output()
        .expect("cargo-about failed")
        .stdout;
    File::create(
        PathBuf::from(env::var("OUT_DIR").expect("no out dir env var"))
            .join("third_party_licenses.html"),
    )
    .expect("failed to create file")
    .write_all(&license_html)
    .expect("failed to create file");
}
