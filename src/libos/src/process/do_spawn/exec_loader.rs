use super::super::elf_file::*;
use super::ThreadRef;
use crate::fs::{FileMode, INodeExt};
use crate::prelude::*;
use rcore_fs::vfs::{FileType, INode, Metadata};
use std::ffi::CString;

/// Load an ELF file header or a script's interpreter header into a vector.
///
/// If the file is an executable binary, then just load this file's header.
/// If the file is an script text, then parse the shebang and load
/// the interpreter header.
pub fn load_exec_file_hdr_to_vec(
    file_path: &str,
    current_ref: &ThreadRef,
) -> Result<(Option<String>, Arc<dyn INode>, Vec<u8>, ElfHeader)> {
    let (inode, file_buf, elf_hdr) = load_file_hdr_to_vec(&file_path, current_ref)?;
    if elf_hdr.is_some() {
        Ok((None, inode, file_buf, elf_hdr.unwrap()))
    } else {
        // loaded file is not Elf format, try script file
        if !is_script_file(&file_buf) {
            return_errno!(ENOEXEC, "unknown executable file format");
        }
        // load interpreter
        let interpreter_path = parse_script_interpreter(&file_buf)?;
        if interpreter_path.starts_with("/host/") {
            return_errno!(
                EACCES,
                "libos doesn't support executing binaries from \"/host\" directory"
            );
        }
        let (interp_inode, interp_buf, interp_hdr) =
            load_file_hdr_to_vec(&interpreter_path, current_ref)?;
        let interp_hdr = if interp_hdr.is_none() {
            return_errno!(ENOEXEC, "scrip interpreter is not ELF format");
        } else {
            interp_hdr.unwrap()
        };
        Ok((Some(interpreter_path), interp_inode, interp_buf, interp_hdr))
    }
}

fn is_script_file(file_buf: &Vec<u8>) -> bool {
    file_buf.starts_with(&[b'#', b'!'])
}

// TODO: Support parsing interpreter arguments. e.g. `/usr/bin/python -u`
fn parse_script_interpreter(file_buf: &Vec<u8>) -> Result<String> {
    let mut start = 2; // after '#', '!'
    const MAX_LEN: usize = 127;

    // skip whitespaced between shebang and interpreter
    while (start < file_buf.len())
        && (file_buf[start] == ' ' as u8 || file_buf[start] == '\t' as u8)
    {
        start += 1;
    }

    let end = file_buf
        .iter()
        .take(MAX_LEN)
        .position(|&c| c == '\n' as u8)
        .ok_or_else(|| errno!(EINVAL, "script parsing error"))?;

    let interpreter = std::str::from_utf8(&file_buf[start..end])
        .map_err(|e| errno!(ENOEXEC, "failed to get the script interpreter"))?;
    trace!("script file using interpreter: {:?}", interpreter);
    Ok(interpreter.to_owned())
}

pub fn load_file_hdr_to_vec(
    file_path: &str,
    current_ref: &ThreadRef,
) -> Result<(Arc<dyn INode>, Vec<u8>, Option<ElfHeader>)> {
    let inode = current_ref
        .fs()
        .read()
        .unwrap()
        .lookup_inode(file_path)
        .map_err(|e| errno!(e.errno(), "cannot find the file"))?;

    // Make sure the final file to exec is not a directory
    let metadata = inode.metadata()?;
    if metadata.type_ != FileType::File {
        return_errno!(EACCES, "it is not a regular file");
    }

    let file_mode = {
        let info = inode.metadata()?;
        FileMode::from_bits_truncate(info.mode)
    };
    if !file_mode.is_executable() {
        return_errno!(EACCES, "file is not executable");
    }
    if file_mode.has_set_uid() || file_mode.has_set_gid() {
        warn!(
            "set-user-ID and set-group-ID are not supportted, FileMode:{:?}",
            file_mode
        );
    }

    // Try to read the file as ELF64
    let mut file_buf = inode
        .read_elf64_lazy_as_vec()
        .map_err(|e| errno!(e.errno(), "failed to read the file"))?;

    let elf_header = ElfFile::parse_elf_hdr(&inode, &mut file_buf);
    if let Ok(elf_header) = elf_header {
        Ok((inode, file_buf, Some(elf_header)))
    } else {
        // this file is not ELF format or there is something wrong when parsing
        warn!("parse elf header error = {}", elf_header.err().unwrap());
        Ok((inode, file_buf, None))
    }
}
