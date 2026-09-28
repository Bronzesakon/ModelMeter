//! Windows DPAPI 凭据加解密。
//!
//! 使用系统 CryptProtectData / CryptUnprotectData 以当前 Windows 用户身份加密，
//! 存储格式为 `enc1:<hex>`，无前缀视为明文（向后兼容旧版本数据）。

use crate::error::AppError;

#[cfg(target_os = "windows")]
#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[cfg(target_os = "windows")]
#[link(name = "crypt32")]
extern "system" {
    fn CryptProtectData(
        pdata_in: *const DataBlob,
        sz_data_descr: *const u16,
        p_optional_entropy: *const DataBlob,
        pv_reserved: *mut core::ffi::c_void,
        p_prompt_struct: *const core::ffi::c_void,
        dw_flags: u32,
        pdata_out: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        pdata_in: *const DataBlob,
        p_sz_data_descr: *mut *mut u16,
        p_optional_entropy: *const DataBlob,
        pv_reserved: *mut core::ffi::c_void,
        p_prompt_struct: *const core::ffi::c_void,
        dw_flags: u32,
        pdata_out: *mut DataBlob,
    ) -> i32;
}

#[cfg(target_os = "windows")]
#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(h_mem: isize) -> isize;
}

#[cfg(target_os = "windows")]
fn dpapi_encrypt(plain: &[u8]) -> Result<Vec<u8>, AppError> {
    use std::ptr;
    let data_in = DataBlob {
        cb_data: plain.len() as u32,
        pb_data: plain.as_ptr() as *mut u8,
    };
    let mut data_out = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };
    // SAFETY: CryptProtectData 读取 data_in 并写入 data_out，两个结构体在调用期间均有效。
    let result = unsafe {
        CryptProtectData(
            &data_in,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            0,
            &mut data_out,
        )
    };
    if result == 0 {
        return Err(AppError::Crypto("DPAPI 加密失败".to_string()));
    }
    // SAFETY: 成功后 data_out 由系统分配且有效，立即拷贝并释放。
    let encrypted = unsafe {
        std::slice::from_raw_parts(data_out.pb_data, data_out.cb_data as usize).to_vec()
    };
    unsafe {
        LocalFree(data_out.pb_data as isize);
    }
    Ok(encrypted)
}

#[cfg(target_os = "windows")]
fn dpapi_decrypt(encrypted: &[u8]) -> Result<Vec<u8>, AppError> {
    use std::ptr;
    let data_in = DataBlob {
        cb_data: encrypted.len() as u32,
        pb_data: encrypted.as_ptr() as *mut u8,
    };
    let mut data_out = DataBlob {
        cb_data: 0,
        pb_data: ptr::null_mut(),
    };
    // SAFETY: CryptUnprotectData 读取 data_in（有效密文）并写入 data_out。
    let result = unsafe {
        CryptUnprotectData(
            &data_in,
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            0,
            &mut data_out,
        )
    };
    if result == 0 {
        return Err(AppError::Crypto(
            "DPAPI 解密失败，凭据可能由其他 Windows 用户加密".to_string(),
        ));
    }
    // SAFETY: 成功后 data_out 由系统分配且有效，立即拷贝并释放。
    let decrypted = unsafe {
        std::slice::from_raw_parts(data_out.pb_data, data_out.cb_data as usize).to_vec()
    };
    unsafe {
        LocalFree(data_out.pb_data as isize);
    }
    Ok(decrypted)
}

#[cfg(not(target_os = "windows"))]
fn dpapi_encrypt(_plain: &[u8]) -> Result<Vec<u8>, AppError> {
    Err(AppError::Crypto("DPAPI 仅在 Windows 上可用".to_string()))
}

#[cfg(not(target_os = "windows"))]
fn dpapi_decrypt(_encrypted: &[u8]) -> Result<Vec<u8>, AppError> {
    Err(AppError::Crypto("DPAPI 仅在 Windows 上可用".to_string()))
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, AppError> {
    if hex.len() % 2 != 0 {
        return Err(AppError::Crypto("十六进制编码长度无效".to_string()));
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&hex[i..i + 2], 16)
                .map_err(|e| AppError::Crypto(format!("无效的十六进制编码: {}", e)))
        })
        .collect()
}

/// 加密凭据为 `enc1:<hex>` 字符串。
///
/// 非 Windows 平台或加密失败时，返回错误由调用方决定是否降级为明文。
pub fn encrypt_credential(plain: &str) -> Result<String, AppError> {
    let encrypted = dpapi_encrypt(plain.as_bytes())?;
    Ok(format!("enc1:{}", hex_encode(&encrypted)))
}

/// 解密凭据。无 `enc1:` 前缀视为明文（向后兼容旧版本数据）直接返回。
pub fn decrypt_credential(stored: &str) -> Result<String, AppError> {
    if let Some(hex) = stored.strip_prefix("enc1:") {
        let encrypted = hex_decode(hex)?;
        let decrypted = dpapi_decrypt(&encrypted)?;
        String::from_utf8(decrypted).map_err(|e| AppError::Crypto(format!("解密内容非法 UTF-8: {}", e)))
    } else {
        Ok(stored.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypt_passthrough_plain() {
        assert_eq!(decrypt_credential("plain-text-token").unwrap(), "plain-text-token");
    }

    #[test]
    fn decrypt_invalid_hex() {
        assert!(decrypt_credential("enc1:invalid_hex").is_err());
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let encrypted = encrypt_credential("secret-token-中文").unwrap();
        assert!(encrypted.starts_with("enc1:"));
        assert_eq!(decrypt_credential(&encrypted).unwrap(), "secret-token-中文");
    }
}