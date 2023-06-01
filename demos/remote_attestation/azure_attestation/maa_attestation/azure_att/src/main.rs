use crate::maa::{maa_attestation, maa_generate_json};

pub mod maa;

const ATTESTATION_PROVIDER_URL: &str = "https://shareduks.uks.attest.azure.net";

fn main() {
    // Sample enclave held data
    let ehd: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

    let maa_json = maa_generate_json(&ehd).unwrap();
    println!("maa json: {}", maa_json);

    let response = maa_attestation(String::from(ATTESTATION_PROVIDER_URL), maa_json).unwrap();
    println!("response: {}", response);
}
