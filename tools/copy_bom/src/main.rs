#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate elf;
extern crate env_logger;
extern crate regex;
extern crate shellexpand;
use bom::Bom;
use clap::{App, Arg, ArgMatches};
use env_logger::Env;

mod bom;
mod error;
mod util;

/// The command line options
#[derive(Debug, Clone)]
struct CopyBomOption {
    // The top bom file
    bom_file: String,
    // the root dir where we try to copy files to
    root_dir: String,
    // set dry run mode. If this flag is set, no real options will done
    dry_run: bool,
    // indicate which dirs to find included bom files
    included_dirs: Vec<String>,
}

impl CopyBomOption {
    // use clap to parse command lines options
    fn parse_command_line() -> Self {
        let arg_matches = read_command_line_options();
        // unwrap can never fail
        let bom_file = arg_matches
            .value_of("bom-file")
            .map(|s| s.to_string())
            .unwrap();
        let root_dir = arg_matches
            .value_of("root-dir")
            .map(|s| s.to_string())
            .unwrap();
        let dry_run = arg_matches.is_present("dry-run");
        let included_dirs = match arg_matches.values_of("include-dirs") {
            None => Vec::new(),
            Some(values) => values.into_iter().map(|s| s.to_string()).collect(),
        };
        CopyBomOption {
            bom_file,
            root_dir,
            dry_run,
            included_dirs,
        }
    }

    /// copy files based on command line options
    fn copy_files(&self) {
        let CopyBomOption {
            bom_file,
            root_dir,
            dry_run,
            included_dirs,
        } = self;
        let image = Bom::from_yaml_file(bom_file);
        image.manage_top_bom(bom_file, root_dir, *dry_run, included_dirs);
    }
}

/// use clap to read command line options
fn read_command_line_options<'a>() -> ArgMatches<'a> {
    App::new("copy_bom")
        .version("v0.1")
        .about("copy files described in a bom file to a given dest root dir")
        .arg(
            Arg::with_name("bom-file")
                .short("f")
                .long("file")
                .required(true)
                .takes_value(true)
                .help("Set the bom file to copy"),
        )
        .arg(
            Arg::with_name("root-dir")
                .long("root")
                .required(true)
                .takes_value(true)
                .help("The dest root dir"),
        )
        .arg(
            Arg::with_name("dry-run")
                .long("dry-run")
                .help("Dry run mode"),
        )
        .arg(
            Arg::with_name("include-dirs")
                .long("include-dir")
                .short("i")
                .multiple(true)
                .takes_value(true)
                .help("Set the paths where to find included bom files"),
        )
        .get_matches()
}

fn main() {
    // the copy_bom log environmental variable
    let env = Env::new().filter("OCCLUM_LOG_LEVEL");
    env_logger::init_from_env(env);

    let copy_bom_option = CopyBomOption::parse_command_line();
    copy_bom_option.copy_files();
}
