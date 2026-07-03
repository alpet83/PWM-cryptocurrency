// Shared Windows PE resource helper for PWM operator binaries.

#[cfg(target_os = "windows")]
fn parse_file_version(raw: &str) -> u64 {
    let mut nums = raw.split(['.', '-']).filter_map(|part| part.parse::<u16>().ok());
    let major = nums.next().unwrap_or(0);
    let minor = nums.next().unwrap_or(0);
    let patch = nums.next().unwrap_or(0);
    let build = nums.next().unwrap_or(0);
    ((major as u64) << 48) | ((minor as u64) << 32) | ((patch as u64) << 16) | (build as u64)
}

pub fn configure_windows_resource(file_desc: &str) {
    println!("cargo:rerun-if-changed=../../assets/branding/pwm.ico");
    println!("cargo:rerun-if-changed=../../build/windows_resource.rs");

    #[cfg(target_os = "windows")]
    {
        use std::{env, path::PathBuf};
        use winresource::{VersionInfo, WindowsResource};

        let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_owned());
        let icon_path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
            .join("../../assets/branding/pwm.ico");

        let mut resource = WindowsResource::new();
        resource.set_icon(icon_path.to_string_lossy().as_ref());
        resource.set("ProductName", "PWM MVP v7");
        resource.set("FileDescription", file_desc);
        resource.set("FileVersion", &version);
        resource.set("ProductVersion", &version);
        resource.set("CompanyName", "PWM project");
        resource.set(
            "LegalCopyright",
            "Copyright (c) PWM project contributors. Licensed under MIT.",
        );

        let packed_version = parse_file_version(&version);
        resource.set_version_info(VersionInfo::FILEVERSION, packed_version);
        resource.set_version_info(VersionInfo::PRODUCTVERSION, packed_version);

        if let Err(err) = resource.compile() {
            let msg = err.to_string();
            if !msg.contains("program not found") {
                panic!("failed to compile Windows resource metadata: {err}");
            }
        }
    }
}
