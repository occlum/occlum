extern crate occlum_dcap;
use occlum_dcap::*;
use std::convert::TryFrom;
use std::io::Result;
use std::str;

struct DcapDemo {
    dcap_quote: DcapQuote,
    quote_size: u32,
    quote_buf: Vec<u8>,
    req_data: sgx_report_data_t,
    supplemental_size: u32,
    suppl_buf: Vec<u8>,
}

impl DcapDemo {
    pub fn new(report_data: &str) -> Self {
        let mut dcap = DcapQuote::new().unwrap();
        let quote_size = dcap.get_quote_size().unwrap();
        let supplemental_size = dcap.get_supplemental_data_size().unwrap();
        let quote_buf: Vec<u8> = vec![0; quote_size as usize];
        let suppl_buf: Vec<u8> = vec![0; supplemental_size as usize];
        let mut req_data = sgx_report_data_t::default();

        //fill in the report data array
        for (pos, val) in report_data.as_bytes().iter().enumerate() {
            req_data.d[pos] = *val;
        }

        Self {
            dcap_quote: dcap,
            quote_size: quote_size,
            quote_buf: quote_buf,
            req_data: req_data,
            supplemental_size: supplemental_size,
            suppl_buf: suppl_buf,
        }
    }

    fn dcap_quote_gen(&mut self) -> i32 {
        let ret = self
            .dcap_quote
            .generate_quote(self.quote_buf.as_mut_ptr(), &mut self.req_data)
            .unwrap();
        if ret < 0 {
            println!("DCAP generate quote failed");
        } else {
            println!("DCAP generate quote successfully");
        }

        ret
    }

    // Quote has type `sgx_quote3_t` and is structured as
    // pub struct sgx_quote3_t {
    //     pub header: sgx_quote_header_t,
    //     pub report_body: sgx_report_body_t,
    //     pub signature_data_len: uint32_t,
    //     pub signature_data: [uint8_t; 0],
    // }

    fn dcap_quote_get_report_body(&mut self) -> Result<*const sgx_report_body_t> {
        let report_body_offset = std::mem::size_of::<sgx_quote_header_t>();
        let report_body: *const sgx_report_body_t =
            (self.quote_buf[report_body_offset..]).as_ptr() as _;

        Ok(report_body)
    }

    fn dcap_quote_get_report_data(&mut self) -> Result<*const sgx_report_data_t> {
        let report_body_ptr = self.dcap_quote_get_report_body().unwrap();
        let report_data_ptr = unsafe { &(*report_body_ptr).report_data };

        Ok(report_data_ptr)
    }

    fn dcap_quote_verify(&mut self) -> sgx_ql_qv_result_t {
        let mut quote_verification_result = sgx_ql_qv_result_t::SGX_QL_QV_RESULT_UNSPECIFIED;
        let mut status = 1;

        let mut verify_arg = IoctlVerDCAPQuoteArg {
            quote_buf: self.quote_buf.as_mut_ptr(),
            quote_size: self.quote_size,
            collateral_expiration_status: &mut status,
            quote_verification_result: &mut quote_verification_result,
            supplemental_data_size: self.supplemental_size,
            supplemental_data: self.suppl_buf.as_mut_ptr(),
        };

        let ret = self.dcap_quote.verify_quote(&mut verify_arg).unwrap();
        if ret < 0 {
            println!("DCAP verify quote failed");
        } else {
            println!("DCAP verify quote successfully");
        }

        quote_verification_result
    }

    fn dcap_dump_quote_info(&mut self) {
        let report_body_ptr = self.dcap_quote_get_report_body().unwrap();

        // Dump ISV FAMILY ID
        let family_id = unsafe { (*report_body_ptr).isv_family_id };
        let (fam_id_l, fam_id_h) = family_id.split_at(8);
        let fam_id_l = <&[u8; 8]>::try_from(fam_id_l).unwrap();
        let fam_id_l = u64::from_le_bytes(*fam_id_l);
        let fam_id_h = <&[u8; 8]>::try_from(fam_id_h).unwrap();
        let fam_id_h = u64::from_le_bytes(*fam_id_h);
        println!("\nSGX ISV Family ID:");
        println!("\t Low 8 bytes: 0x{:016x?}\t", fam_id_l);
        println!("\t high 8 bytes: 0x{:016x?}\t", fam_id_h);

        // Dump ISV EXT Product ID
        let prod_id = unsafe { (*report_body_ptr).isv_ext_prod_id };
        let (prod_id_l, prod_id_h) = prod_id.split_at(8);
        let prod_id_l = <&[u8; 8]>::try_from(prod_id_l).unwrap();
        let prod_id_l = u64::from_le_bytes(*prod_id_l);
        let prod_id_h = <&[u8; 8]>::try_from(prod_id_h).unwrap();
        let prod_id_h = u64::from_le_bytes(*prod_id_h);
        println!("\nSGX ISV EXT Product ID:");
        println!("\t Low 8 bytes: 0x{:016x?}\t", prod_id_l);
        println!("\t high 8 bytes: 0x{:016x?}\t", prod_id_h);

        // Dump CONFIG ID
        let conf_id = unsafe { (*report_body_ptr).config_id };
        println!("\nSGX CONFIG ID:");
        println!("\t{:02x?}", &conf_id[..16]);
        println!("\t{:02x?}", &conf_id[16..32]);
        println!("\t{:02x?}", &conf_id[32..48]);
        println!("\t{:02x?}", &conf_id[48..]);

        // Dump CONFIG SVN
        let conf_svn = unsafe { (*report_body_ptr).config_svn };
        println!("\nSGX CONFIG SVN:\t {:04x?}", conf_svn);
    }
}

impl Drop for DcapDemo {
    fn drop(&mut self) {
        self.dcap_quote.close();
    }
}

fn main() {
    let report_str = "Dcap demo sample";
    let mut dcap_demo = DcapDemo::new(report_str);

    println!("Generate quote with report data : {}", report_str);
    dcap_demo.dcap_quote_gen();

    // compare the report data in quote buffer
    let report_data_ptr = dcap_demo.dcap_quote_get_report_data().unwrap();
    let string = str::from_utf8(unsafe { &(*report_data_ptr).d }).unwrap();

    if report_str == &string[..report_str.len()] {
        println!("Report data from Quote: '{}' exactly matches.", string);
    } else {
        println!("Report data from Quote: '{}' doesn't match !!!", string);
    }

    dcap_demo.dcap_dump_quote_info();

    let result = dcap_demo.dcap_quote_verify();
    match result {
        sgx_ql_qv_result_t::SGX_QL_QV_RESULT_OK => {
            println!("Succeed to verify the quote!");
        }
        sgx_ql_qv_result_t::SGX_QL_QV_RESULT_CONFIG_NEEDED
        | sgx_ql_qv_result_t::SGX_QL_QV_RESULT_OUT_OF_DATE
        | sgx_ql_qv_result_t::SGX_QL_QV_RESULT_OUT_OF_DATE_CONFIG_NEEDED
        | sgx_ql_qv_result_t::SGX_QL_QV_RESULT_SW_HARDENING_NEEDED
        | sgx_ql_qv_result_t::SGX_QL_QV_RESULT_CONFIG_AND_SW_HARDENING_NEEDED => {
            println!(
                "WARN: App: Verification completed with Non-terminal result: {:?}",
                result
            );
        }
        _ => println!(
            "Error: App: Verification completed with Terminal result: {:?}",
            result
        ),
    }
}
