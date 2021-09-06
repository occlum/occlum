use super::*;
use crate::misc;
use rcore_fs_sefs::dev::{SefsUuid, UuidProvider};

pub struct SgxUuidProvider;

impl UuidProvider for SgxUuidProvider {
    fn generate_uuid(&self) -> SefsUuid {
        let mut uuid: [u8; 16] = [0u8; 16];
        misc::get_random(&mut uuid).expect("failed to get random number");
        SefsUuid(uuid)
    }
}
