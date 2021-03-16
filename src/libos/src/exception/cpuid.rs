use super::*;
use crate::syscall::CpuContext;
use sgx_types::*;
use std::collections::HashMap;
use std::rsgx_cpuidex;

pub const CPUID_OPCODE: u16 = 0xA20F;
const CPUID_MIN_BASIC_LEAF: u32 = 0;
const CPUID_MIN_EXTEND_LEAF: u32 = 0x8000_0000;
const CPUID_MAX_SUBLEAF: u32 = u32::max_value();

#[repr(C)]
#[derive(Eq, PartialEq, Hash, Clone, Copy)]
struct CpuIdInput {
    leaf: u32,
    subleaf: u32,
}

#[repr(C)]
#[derive(Eq, PartialEq, Hash, Clone, Copy, Debug)]
struct CpuIdResult {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
}

struct CpuIdCache {
    inner: HashMap<CpuIdInput, CpuIdResult>,
}

struct CpuId {
    cache: CpuIdCache,
    max_basic_leaf: u32,
    max_extend_leaf: u32,
}

impl CpuIdCache {
    pub fn new(max_basic_leaf: u32, max_extend_leaf: u32) -> CpuIdCache {
        let mut cache = CpuIdCache {
            inner: HashMap::new(),
        };
        cache.generate_cpuid_cache(max_basic_leaf, max_extend_leaf);
        cache
    }

    pub fn lookup(&self, key: &CpuIdInput) -> Option<&CpuIdResult> {
        self.inner.get(key)
    }

    fn insert(&mut self, key: CpuIdInput, value: CpuIdResult) {
        // If EAX/EBX/ECX/EDX return 0, dismiss it
        if (value.eax | value.ebx | value.ecx | value.edx) != 0 {
            self.inner.insert(key, value);
        }
    }

    fn generate_cpuid_cache(&mut self, max_basic_leaf: u32, max_extend_leaf: u32) {
        let mut sgx_support: bool = false;
        // Generate basic leaf cpuid cache
        for leaf in CPUID_MIN_BASIC_LEAF..=max_basic_leaf {
            // Intel SGX Capability Enumeration Leaf,
            // Leaf 12H sub-leaf 0 is supported if CPUID.(EAX=07H, ECX=0H):EBX[SGX] = 1.
            if leaf == 0x12 && !sgx_support {
                continue;
            }
            let mut max_subleaf = 0;
            for subleaf in (0..) {
                let cpuid_input = CpuIdInput { leaf, subleaf };
                let cpuid_result = get_cpuid_info_via_ocall(cpuid_input);
                self.insert(cpuid_input, cpuid_result);
                // Most leaf only supports (sub-leaf == 0), and many others can determine their
                // maximum supported sub-leaf according to CPUID.(EAX=Leaf, ECX=0H).
                if subleaf == 0 {
                    max_subleaf = match leaf {
                        // EAX Bits 31 - 00: Reports the maximum sub-leaf supported.
                        0x7 | 0x14 | 0x17 | 0x18 => cpuid_result.eax,
                        // Reports valid resource type starting at bit position 1 of EDX.
                        // EDX Bit 00: Reserved.
                        //     Bit 01: Supports L3 Cache Intel RDT Monitoring if 1.
                        //     Bits 31 - 02: Reserved.
                        0xF => (cpuid_result.edx & 0x0000_0002) >> 1,
                        // Reports valid ResID starting at bit position 1 of EBX.
                        // EBX Bit 00: Reserved.
                        //     Bit 01: Supports L3 Cache Allocation Technology if 1.
                        //     Bit 02: Supports L2 Cache Allocation Technology if 1.
                        //     Bit 03: Supports Memory Bandwidth Allocation if 1.
                        //     Bits 31 - 04: Reserved.
                        0x10 => match cpuid_result.ebx & 0x0000_000E {
                            0x0000_0008 | 0x0000_000A | 0x0000_000C | 0x0000_000E => 3,
                            0x0000_0004 | 0x0000_0006 => 2,
                            0x0000_0002 => 1,
                            _ => 0,
                        },
                        // Processor Extended State Enumeration, Sub-leaf n (0 ≤ n ≤ 63)
                        0xD => 63,
                        // (Sub-leaf == 0) can not decide max_subleaf for these leaf,
                        // later match expression will decide the max_subleaf.
                        0x4 | 0xB | 0x12 | 0x1F => CPUID_MAX_SUBLEAF,
                        // Default max_subleaf is 0.
                        _ => 0,
                    };
                    if leaf == 0x7 {
                        // EBX Bit 02: Supports Intel® SGX Extensions if 1.
                        sgx_support = (cpuid_result.ebx & 0x0000_0004) != 0;
                    }
                }
                // These leafs determine the maximum supported sub-leaf according to
                // the output of CPUID instruction at every iteration.
                if max_subleaf == CPUID_MAX_SUBLEAF {
                    max_subleaf = match leaf {
                        // Deterministic Cache Parameters Leaf
                        // Sub-leaf index n+1 is invalid if subleaf n returns EAX[4:0] as 0.
                        0x4 if (cpuid_result.eax & 0x1F) == 0 => subleaf,
                        // Extended Topology Enumeration Leaf
                        // If an input value n in ECX returns the invalid level-type of 0 in ECX[15:8],
                        // other input values with ECX > n also return 0 in ECX[15:8].
                        0xB if (cpuid_result.ecx & 0x0000_FF00) == 0 => subleaf,
                        // Intel SGX EPC Enumeration Leaf
                        // EAX Bit 03 - 00: Sub-leaf Type.
                        //         0000b: Indicates this sub-leaf is invalid.
                        0x12 if subleaf >= 2 && (cpuid_result.eax & 0x0000000F) == 0 => subleaf,
                        // V2 Extended Topology Enumeration Leaf
                        // CPUID leaf 0x1F is a preferred superset to leaf 0xB.
                        0x1F if (cpuid_result.ecx & 0x0000_FF00) == 0 => subleaf,
                        // Condition not met.
                        _ => max_subleaf,
                    };
                }
                if subleaf == max_subleaf {
                    break;
                }
            }
        }
        // Generate extend leaf cpuid cache
        for leaf in CPUID_MIN_EXTEND_LEAF..=max_extend_leaf {
            let cpuid_input = CpuIdInput { leaf, subleaf: 0 };
            let cpuid_result = get_cpuid_info_via_ocall(cpuid_input);
            self.insert(cpuid_input, cpuid_result);
        }
    }
}

impl CpuId {
    pub fn new() -> CpuId {
        let max_basic_leaf = match rsgx_cpuidex(CPUID_MIN_BASIC_LEAF as i32, 0) {
            Ok(sgx_cpuinfo) => sgx_cpuinfo[0] as u32,
            _ => panic!("failed to call sgx_cpuidex"),
        };
        let max_extend_leaf = match rsgx_cpuidex(CPUID_MIN_EXTEND_LEAF as i32, 0) {
            Ok(sgx_cpuinfo) => sgx_cpuinfo[0] as u32,
            _ => panic!("failed to call sgx_cpuidex"),
        };
        let cpuid = CpuId {
            cache: CpuIdCache::new(max_basic_leaf, max_extend_leaf),
            max_basic_leaf,
            max_extend_leaf,
        };
        cpuid
    }

    fn lookup_cpuid_from_cache(&self, cpuid_input: CpuIdInput) -> Result<CpuIdResult> {
        self.cache
            .lookup(&cpuid_input)
            .map(|result| result.clone())
            .ok_or_else(|| errno!(ENOENT, "cpuid_result not found"))
    }

    pub fn get_max_basic_leaf(&self) -> u32 {
        self.max_basic_leaf
    }

    pub fn get_max_extend_leaf(&self) -> u32 {
        self.max_extend_leaf
    }

    pub fn get_cpuid_info(&self, leaf: u32, subleaf: u32) -> CpuIdResult {
        // If a value entered for CPUID.EAX is higher than the maximum input value
        // for basic or extended function for that processor then the data for the
        // highest basic information leaf is returned.
        let fixed_leaf = if (CPUID_MIN_BASIC_LEAF..=self.max_basic_leaf).contains(&leaf)
            || (CPUID_MIN_EXTEND_LEAF..=self.max_extend_leaf).contains(&leaf)
        {
            leaf
        } else {
            self.max_basic_leaf
        };
        let fixed_subleaf = if is_cpuid_leaf_has_subleaves(fixed_leaf) {
            subleaf
        } else {
            0
        };
        let cpuid_input = CpuIdInput {
            leaf: fixed_leaf,
            subleaf: fixed_subleaf,
        };
        let cpuid_result = match self.lookup_cpuid_from_cache(cpuid_input) {
            Ok(cpuid_result) => cpuid_result,
            // If a value entered for CPUID.EAX is less than or equal to the maximum input value
            // and the leaf is not supported on that processor then 0 is returned in all the registers.
            Err(error) => CpuIdResult {
                eax: 0,
                ebx: 0,
                ecx: 0,
                edx: 0,
            },
        };
        cpuid_result
    }
}

lazy_static! {
    static ref CPUID: CpuId = CpuId::new();
}

fn is_cpuid_leaf_has_subleaves(leaf: u32) -> bool {
    const CPUID_LEAF_WITH_SUBLEAF: [u32; 11] =
        [0x4, 0x7, 0xB, 0xD, 0xF, 0x10, 0x12, 0x14, 0x17, 0x18, 0x1F];
    CPUID_LEAF_WITH_SUBLEAF.contains(&leaf)
}

// We cannot do OCalls when handling exceptions. So this function is only useful
// when we call setup_cpuid_info to initialize the CPUID singleton,
// which caches cpuid info for use when handling cpuid exception.
fn get_cpuid_info_via_ocall(cpuid_input: CpuIdInput) -> CpuIdResult {
    let cpuid_result = match rsgx_cpuidex(cpuid_input.leaf as i32, cpuid_input.subleaf as i32) {
        Ok(sgx_cpuinfo) => CpuIdResult {
            eax: sgx_cpuinfo[0] as u32,
            ebx: sgx_cpuinfo[1] as u32,
            ecx: sgx_cpuinfo[2] as u32,
            edx: sgx_cpuinfo[3] as u32,
        },
        _ => panic!("failed to call sgx_cpuidex"),
    };
    cpuid_result
}

pub fn setup_cpuid_info() {
    // Make lazy_static to be executed at runtime in order to be initialized
    let max_basic_leaf = CPUID.get_max_basic_leaf();
}

pub fn handle_cpuid_exception(user_context: &mut CpuContext) -> Result<isize> {
    debug!("handle CPUID exception");
    let leaf = user_context.rax as u32;
    let subleaf = user_context.rcx as u32;
    let cpuid_result = CPUID.get_cpuid_info(leaf, subleaf);
    trace!("cpuid result: {:?}", cpuid_result);
    user_context.rax = cpuid_result.eax as u64;
    user_context.rbx = cpuid_result.ebx as u64;
    user_context.rcx = cpuid_result.ecx as u64;
    user_context.rdx = cpuid_result.edx as u64;
    user_context.rip += 2;

    Ok(0)
}
