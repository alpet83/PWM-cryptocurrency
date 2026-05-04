//! User-visible fatal CLI errors: message on stderr, exit code 2.

use std::process;

pub(crate) fn exit_user_error(msg: &str) -> ! {
    eprintln!("{msg}");
    process::exit(2);
}
