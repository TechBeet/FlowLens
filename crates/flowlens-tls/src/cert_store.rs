//! Windows Certificate Store operations for CA installation/uninstallation
//! Uses Windows CryptoAPI (crypt32.dll) to manage certificates.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows_sys::Win32::Foundation::FALSE;
use windows_sys::Win32::Security::Cryptography::*;

use crate::TlsError;

const CERT_SYSTEM_STORE_LOCAL_MACHINE: u32 = 0x00080000;
const CERT_SYSTEM_STORE_CURRENT_USER: u32 = 0x00010000;

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, TlsError> {
    let pem_content = pem
        .lines()
        .filter(|line| !line.starts_with("-----"))
        .collect::<Vec<_>>()
        .join("");
    base64_decode(&pem_content)
        .map_err(|e| TlsError::CertGeneration(format!("Failed to decode PEM: {}", e)))
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let mut i = 0;
    let bytes = input.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'=' {
            break;
        }
        let b0 = b64_val(bytes.get(i).copied().unwrap_or(b'A'))?;
        let b1 = b64_val(bytes.get(i + 1).copied().unwrap_or(b'A'))?;
        let b2 = b64_val(bytes.get(i + 2).copied().unwrap_or(b'A'))?;
        let b3 = b64_val(bytes.get(i + 3).copied().unwrap_or(b'A'))?;
        result.push((b0 << 2) | (b1 >> 4));
        if i + 2 < bytes.len() && bytes[i + 2] != b'=' {
            result.push((b1 << 4) | (b2 >> 2));
        }
        if i + 3 < bytes.len() && bytes[i + 3] != b'=' {
            result.push((b2 << 6) | b3);
        }
        i += 4;
    }
    Ok(result)
}

fn b64_val(c: u8) -> Result<u8, String> {
    match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        b'=' => Ok(0),
        _ if c.is_ascii_whitespace() => Ok(0),
        _ => Err(format!("Invalid base64 char: {}", c as char)),
    }
}

fn get_last_error() -> String {
    unsafe {
        format!(
            "Win32 error code: {}",
            windows_sys::Win32::Foundation::GetLastError()
        )
    }
}

unsafe fn open_cert_store(store_name: &str, scope: u32) -> Result<*mut std::ffi::c_void, TlsError> {
    let wide_name = to_wide(store_name);
    let handle = CertOpenStore(
        CERT_STORE_PROV_SYSTEM_W,
        0,
        0,
        scope,
        wide_name.as_ptr() as *const _,
    );
    if handle.is_null() {
        Err(TlsError::CertGeneration(format!(
            "Failed to open cert store '{}': {}",
            store_name,
            get_last_error()
        )))
    } else {
        Ok(handle)
    }
}

unsafe fn close_cert_store(handle: *mut std::ffi::c_void) {
    if !handle.is_null() {
        CertCloseStore(handle, 0);
    }
}

/// Install a CA certificate to the Windows Trusted Root store
pub fn install_ca_to_store(pem: &str) -> Result<(), TlsError> {
    let der_bytes = pem_to_der(pem)?;
    unsafe {
        let store_handle = open_cert_store("Root", CERT_SYSTEM_STORE_LOCAL_MACHINE)
            .or_else(|_| open_cert_store("Root", CERT_SYSTEM_STORE_CURRENT_USER))?;
        let result = CertAddEncodedCertificateToStore(
            store_handle,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            der_bytes.as_ptr(),
            der_bytes.len() as u32,
            CERT_STORE_ADD_REPLACE_EXISTING,
            std::ptr::null_mut(),
        );
        close_cert_store(store_handle);
        if result == FALSE {
            Err(TlsError::CertGeneration(format!(
                "Failed to add cert: {}",
                get_last_error()
            )))
        } else {
            Ok(())
        }
    }
}

/// Uninstall a CA certificate from the Windows certificate stores
pub fn uninstall_ca_from_store(pem: &str) -> Result<(), TlsError> {
    let der_bytes = pem_to_der(pem)?;
    unsafe {
        let scopes = [
            CERT_SYSTEM_STORE_LOCAL_MACHINE,
            CERT_SYSTEM_STORE_CURRENT_USER,
        ];
        for scope in scopes {
            let store_handle = match open_cert_store("Root", scope) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let mut context = CertFindCertificateInStore(
                store_handle,
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                0,
                CERT_FIND_ANY,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            while !context.is_null() {
                let cert_encoding = (*context).pbCertEncoded;
                let cert_len = (*context).cbCertEncoded;
                if cert_len == der_bytes.len() as u32
                    && der_bytes
                        .as_slice()
                        .iter()
                        .zip(std::slice::from_raw_parts(cert_encoding, cert_len as usize).iter())
                        .all(|(a, b)| a == b)
                {
                    CertDeleteCertificateFromStore(context);
                    close_cert_store(store_handle);
                    return Ok(());
                }
                let next = CertFindCertificateInStore(
                    store_handle,
                    X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                    0,
                    CERT_FIND_ANY,
                    context as *const _ as *const std::ffi::c_void,
                    std::ptr::null_mut(),
                );
                CertFreeCertificateContext(context);
                context = next;
            }
            close_cert_store(store_handle);
        }
    }
    Ok(())
}

/// Check if a CA certificate is installed in the Windows certificate stores
pub fn is_ca_installed(pem: &str) -> bool {
    let der_bytes = match pem_to_der(pem) {
        Ok(b) => b,
        Err(_) => return false,
    };
    unsafe {
        let scopes = [
            CERT_SYSTEM_STORE_LOCAL_MACHINE,
            CERT_SYSTEM_STORE_CURRENT_USER,
        ];
        for scope in scopes {
            let store_handle = match open_cert_store("Root", scope) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let mut context = CertFindCertificateInStore(
                store_handle,
                X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                0,
                CERT_FIND_ANY,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            while !context.is_null() {
                let cert_encoding = (*context).pbCertEncoded;
                let cert_len = (*context).cbCertEncoded;
                if cert_len == der_bytes.len() as u32 {
                    let mut matches = true;
                    for (i, b) in der_bytes.iter().enumerate() {
                        if *cert_encoding.add(i) != *b {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        CertFreeCertificateContext(context);
                        close_cert_store(store_handle);
                        return true;
                    }
                }
                let next = CertFindCertificateInStore(
                    store_handle,
                    X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                    0,
                    CERT_FIND_ANY,
                    context as *const _ as *const std::ffi::c_void,
                    std::ptr::null_mut(),
                );
                CertFreeCertificateContext(context);
                context = next;
            }
            close_cert_store(store_handle);
        }
        false
    }
}
