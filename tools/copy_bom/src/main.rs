#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate elf;
extern crate env_logger;
extern crate regex;
extern crate shellexpand;
extern crate walkdir;
use bom::Bom;
use env_logger::Env;
use structopt::StructOpt;
use util::check_rsync;

mod bom;
mod error;
mod util;

/// copy files described in a bom file to a given dest root dir
#[derive(Debug, Clone, StructOpt)]
struct CopyBomOption {
    /// Set the bom file to copy
    #[structopt(short = "f", long = "file")]
    bom_file: String,
    /// The dest root dir
    #[structopt(long = "root")]
    root_dir: String,
    /// Dry run mode
    #[structopt(long = "dry-run")]
    dry_run: bool,
    /// Set the paths where to find included bom files
    #[structopt(long = "include-dir")]
    included_dirs: Vec<String>,
    /// Set the paths under where to log files
    #[structopt(long = "filter-path")]
    filter_path: Option<String>,
}

impl CopyBomOption {
    fn copy_files(&self) {
        let CopyBomOption {
            bom_file,
            root_dir,
            dry_run,
            included_dirs,
            filter_path: _,
        } = self;
        let image = Bom::from_yaml_file(bom_file);
        image.manage_top_bom(bom_file, root_dir, *dry_run, included_dirs);
    }
}

fn main() {
    // the copy_bom log environmental variable
    let env = Env::new().filter("OCCLUM_LOG_LEVEL");
    env_logger::init_from_env(env);
    check_rsync();

    let copy_bom_option = CopyBomOption::from_args();
    if copy_bom_option.filter_path.is_some(){
        util::set_log_filter_path(copy_bom_option.filter_path.clone());
    }
    copy_bom_option.copy_files();
}
