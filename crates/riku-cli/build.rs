//! Passes the target triple to the crate as `RIKU_TARGET`, so plugin
//! downloads fetch the build that matches this binary (armv6 vs armv7, musl
//! vs glibc), which `std::env::consts` cannot tell apart.

fn main() {
    let target = match std::env::var("TARGET") {
        Ok(target) => target,
        Err(e) => panic!("cargo did not set TARGET for the build script: {e}"),
    };
    println!("cargo:rustc-env=RIKU_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");
}
