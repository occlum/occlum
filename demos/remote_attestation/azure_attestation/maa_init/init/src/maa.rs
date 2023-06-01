use occlum_dcap::*;
use reqwest::blocking::Client;
use serde_json::json;
use sha2::{Digest, Sha256};

pub const MAX_REPORT_DATA_SIZE: usize = 64;

fn maa_get_quote_base64(user_data: &[u8]) -> Result<String, &'static str> {
    let mut dcap = DcapQuote::new().unwrap();
    let quote_size = dcap.get_quote_size().unwrap();
    let mut quote_buf: Vec<u8> = vec![0; quote_size as usize];
    let mut report_data = sgx_report_data_t::default();

    //fill in the report data array
    let len = {
        if user_data.len() > MAX_REPORT_DATA_SIZE {
            MAX_REPORT_DATA_SIZE
        } else {
            user_data.len()
        }
    };

    for i in 0..len {
        report_data.d[i] = user_data[i];
    }

    let ret = dcap
        .generate_quote(quote_buf.as_mut_ptr(), &mut report_data)
        .unwrap();
    dcap.close();
    if ret < 0 {
        return Err("DCAP generate quote failed");
    }

    let quote = base64::encode(&quote_buf);
    Ok(quote)
}

pub fn maa_generate_json(user_data: &[u8]) -> Result<serde_json::Value, &'static str> {
    let mut hasher = Sha256::new();
    hasher.update(user_data);
    let hash = hasher.finalize();

    let quote_base64 = maa_get_quote_base64(&hash).unwrap();

    // Format to MAA rest attestation API request body
    // https://docs.microsoft.com/en-us/rest/api/attestation/attestation/attest-sgx-enclave#request-body
    let mut maa_json: serde_json::Value = json!({
        "quote": "0",
        "runtimeData": {
            "data": "0",
            "dataType":"Binary"
        }
    });

    *maa_json.pointer_mut("/quote").unwrap() = serde_json::Value::String(quote_base64);

    *maa_json.pointer_mut("/runtimeData/data").unwrap() =
        serde_json::Value::String(base64::encode(&user_data));

    Ok(maa_json.to_owned())
}

pub fn maa_attestation(
    url: String,
    request_body: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let client = Client::new();
    let att_url = format!("{}/attest/SgxEnclave?api-version=2020-10-01", url);

    let resp = client.post(att_url).json(&request_body).send()?;

    match resp.status() {
        reqwest::StatusCode::OK => {
            // println!("success!");
            Ok(resp.json().unwrap())
        }
        s => {
            println!("Received response status: {:?}", s);
            Err("maa attestation failed".into())
        }
    }
}
