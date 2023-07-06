use super::*;

pub fn do_getcwd() -> Result<String> {
    debug!("getcwd");
    let thread = current!();
    let fs = thread.fs();
    let cwd_path = fs.cwd().abs_path();
    Ok(cwd_path)
}
