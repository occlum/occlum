extern crate sgx_cov;
use sgx_cov::*;

global_dtors_object! {
    COV_FINALIZE, cov_exit = {
        cov_writeout();
        eprintln!("Coverage data gathered!");
    }
}
