use std::process::Command;

fn main() {
    // Setting build tag from git repo
    let git_out = Command::new("git").args(["describe", "--tag"]).output();

    match git_out {
        Ok(o) if o.status.success() => {
            let tag = String::from_utf8_lossy(&o.stdout).trim().to_string();
            println!("cargo:warning=GIT tag extracted: {}", tag);
            println!("cargo:rustc-env=GIT_TAG={}", tag);
        }
        Ok(o) => println!(
            "cargo:warning=Git command exited with error: {}",
            String::from_utf8_lossy(&o.stderr).to_string()
        ),
        Err(e) => println!("cargo:warning=Can not extract git tag: {}", e),
    }
}
