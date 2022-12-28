use super::super::elf_file::*;
use super::ThreadRef;
use crate::fs::{AsyncInodeExt, FileMode, FileType, FsPath, Metadata};
use crate::prelude::*;
use std::convert::TryFrom;
use std::ffi::CString;

/// Load an ELF file header or a script's interpreter header into a vector.
///
/// If the file is an executable binary, then just load this file's header.
/// If the file is an script text, then parse the shebang and load
/// the interpreter header.
pub async fn load_exec_file_hdr_to_vec(
    file_path: &str,
    current_ref: &ThreadRef,
) -> Result<(Option<String>, FileRef, Vec<u8>, ElfHeader)> {
    let (file_ref, file_buf, elf_hdr) = load_file_hdr_to_vec(&file_path, current_ref).await?;
    if elf_hdr.is_some() {
        Ok((None, file_ref, file_buf, elf_hdr.unwrap()))
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
        let (interp_file, interp_buf, interp_hdr) =
            load_file_hdr_to_vec(&interpreter_path, current_ref).await?;
        let interp_hdr = if interp_hdr.is_none() {
            return_errno!(ENOEXEC, "scrip interpreter is not ELF format");
        } else {
            interp_hdr.unwrap()
        };
        Ok((Some(interpreter_path), interp_file, interp_buf, interp_hdr))
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

pub async fn load_file_hdr_to_vec(
    file_path: &str,
    current_ref: &ThreadRef,
) -> Result<(FileRef, Vec<u8>, Option<ElfHeader>)> {
    let file_ref = current_ref
        .fs()
        .open_file(&FsPath::try_from(file_path)?, 0, FileMode::S_IRUSR)
        .await?;

    // Make sure the final file to exec is not a directory
    let metadata = file_ref
        .as_async_file_handle()
        .unwrap()
        .dentry()
        .inode()
        .metadata()
        .await?;

    if metadata.type_ != FileType::File {
        return_errno!(EACCES, "it is not a regular file");
    }

    let file_mode = FileMode::from_bits_truncate(metadata.mode);
    if !file_mode.is_executable() {
        return_errno!(EACCES, "file is not executable");
    }
    if file_mode.has_set_uid() || file_mode.has_set_gid() {
        warn!(
            "set-user-ID and set-group-ID are not supported, FileMode:{:?}",
            file_mode
        );
    }

    // Try to read the file as ELF64
    let mut file_buf = file_ref
        .as_async_file_handle()
        .unwrap()
        .dentry()
        .inode()
        .read_elf64_lazy_as_vec()
        .await
        .map_err(|e| errno!(e.errno(), "failed to read the file"))?;

    let elf_header = ElfFile::parse_elf_hdr(&file_ref, &mut file_buf).await;

    if let Ok(elf_header) = elf_header {
        Ok((file_ref, file_buf, Some(elf_header)))
    } else {
        // this file is not ELF format or there is something wrong when parsing
        warn!("parse elf header error = {}", elf_header.err().unwrap());
        Ok((file_ref, file_buf, None))
    }
}
