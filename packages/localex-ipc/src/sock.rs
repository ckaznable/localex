use std::{fs, os::unix::fs::MetadataExt, path::Path};

pub const SOCK_SYSTEM_PATH: &str = "/run/localex.sock";
pub const SOCK_PATH: &str = "/tmp/localex.sock";

fn check_permissions<P: AsRef<Path>>(path: P) -> std::io::Result<bool> {
    let metadata = fs::metadata(path)?;

    let mode = metadata.mode();

    let user_id = metadata.uid();
    let group_id = metadata.gid();

    let user_write = mode & 0o200 != 0;
    let group_write = mode & 0o020 != 0;
    let others_write = mode & 0o002 != 0;

    let current_uid = nix::unistd::getuid().as_raw();
    let current_gid = nix::unistd::getgid().as_raw();

    let has_write_permission = (user_id == current_uid && user_write)
        || (group_id == current_gid && group_write)
        || others_write;

    Ok(has_write_permission)
}

pub fn get_sock_connect_path() -> &'static Path {
    let sys_path = Path::new(SOCK_SYSTEM_PATH);
    if sys_path.exists() {
        sys_path
    } else {
        Path::new(SOCK_PATH)
    }
}

pub fn get_sock_mount_path() -> &'static Path {
    let p = match check_permissions(SOCK_SYSTEM_PATH) {
        Ok(true) => SOCK_SYSTEM_PATH,
        _ => SOCK_PATH,
    };

    Path::new(p)
}

pub fn is_sock_exist() -> bool {
    Path::new(SOCK_SYSTEM_PATH).exists() ||
    Path::new(SOCK_PATH).exists()
}
