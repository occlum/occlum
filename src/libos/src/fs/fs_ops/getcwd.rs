use super::*;

pub fn do_getcwd() -> Result<String> {
    debug!("getcwd");
    let thread = current!();
    let fs = thread.fs();
    let cwd = fs.cwd().to_owned();
    Ok(cwd)
}
