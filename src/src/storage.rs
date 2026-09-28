use serde_json;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

pub struct Storage {
    data_dir: OnceLock<PathBuf>,
    write_lock: Mutex<()>,
}

impl Storage {
    pub fn new() -> Self {
        Self { data_dir: OnceLock::new(), write_lock: Mutex::new(()) }
    }

    pub fn init(&self, data_dir: PathBuf) {
        fs::create_dir_all(&data_dir).ok();
        self.data_dir.set(data_dir).ok();
    }

    fn data_dir(&self) -> &PathBuf {
        self.data_dir.get().expect("Storage not initialized")
    }

    fn settings_path(&self) -> PathBuf {
        self.data_dir().join("settings.json")
    }

    // MARK: - Refresh Interval

    pub fn load_refresh_interval(&self) -> f64 {
        self.load_setting("refresh_interval")
            .and_then(|v| v.parse().ok())
            .unwrap_or(60.0)
    }

    pub fn save_refresh_interval(&self, interval: f64) {
        self.save_setting("refresh_interval", &interval.to_string());
    }

    // MARK: - Edge Snap

    pub fn load_edge_snap_enabled(&self) -> bool {
        self.load_setting("edge_snap_enabled")
            .map(|v| v == "true")
            .unwrap_or(false)
    }

    pub fn save_edge_snap_enabled(&self, enabled: bool) {
        self.save_setting("edge_snap_enabled", if enabled { "true" } else { "false" });
    }

    // MARK: - Platform Token

    pub fn save_platform_token(&self, token: &str) -> io::Result<()> {
        let stored = crate::crypto::encrypt_credential(token)
            .map_err(|_e| {
                crate::debug_log!("[storage] platform_token 加密失败，降级为明文: {}", _e);
            })
            .ok()
            .unwrap_or_else(|| token.to_string());
        let data = serde_json::json!({
            "Token": stored,
            "SavedAt": chrono::Utc::now().to_rfc3339()
        });
        let path = self.data_dir().join("platform_token.json");
        fs::write(path, serde_json::to_string(&data)?)?;
        Ok(())
    }

    pub fn load_platform_token(&self) -> Option<String> {
        let path = self.data_dir().join("platform_token.json");
        if !path.exists() { return None; }
        let json = fs::read_to_string(path).ok()?;
        let doc: serde_json::Value = serde_json::from_str(&json).ok()?;
        let stored = doc.get("Token")?.as_str()?;
        crate::crypto::decrypt_credential(stored).ok()
    }

    pub fn has_saved_platform_token(&self) -> bool {
        self.data_dir().join("platform_token.json").exists()
    }

    // MARK: - Platform Cookies

    pub fn save_platform_cookies(&self, cookies: &str) -> io::Result<()> {
        let stored = crate::crypto::encrypt_credential(cookies)
            .map_err(|_e| {
                crate::debug_log!("[storage] platform_cookies 加密失败，降级为明文: {}", _e);
            })
            .ok()
            .unwrap_or_else(|| cookies.to_string());
        let data = serde_json::json!({
            "CookieHeader": stored,
            "SavedAt": chrono::Utc::now().to_rfc3339()
        });
        let path = self.data_dir().join("platform_cookies.json");
        fs::write(path, serde_json::to_string(&data)?)?;
        Ok(())
    }

    pub fn load_platform_cookies(&self) -> Option<String> {
        let path = self.data_dir().join("platform_cookies.json");
        if !path.exists() { return None; }
        let json = fs::read_to_string(path).ok()?;
        let doc: serde_json::Value = serde_json::from_str(&json).ok()?;
        let stored = doc.get("CookieHeader")?.as_str()?;
        crate::crypto::decrypt_credential(stored).ok()
    }

    // MARK: - Clear

    pub fn clear_platform_token(&self) {
        let path = self.data_dir().join("platform_token.json");
        fs::remove_file(path).ok();
    }

    pub fn clear_platform_cookies(&self) {
        let path = self.data_dir().join("platform_cookies.json");
        fs::remove_file(path).ok();
    }

    pub fn clear_all(&self) -> io::Result<()> {
        let dir = self.data_dir();
        if dir.exists() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                if entry.path().extension().map_or(false, |e| e == "json") {
                    fs::remove_file(entry.path())?;
                }
            }
        }
        Ok(())
    }

 pub fn is_first_launch(&self) -> bool {
        !self.load_onboarding_completed()
    }

    pub fn load_onboarding_completed(&self) -> bool {
        let path = self.settings_path();
        let Ok(content) = std::fs::read_to_string(&path) else {
            return false;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
            return false;
        };
        value.get("onboarding_completed").and_then(|v| v.as_bool()).unwrap_or(false)
    }

    pub fn save_onboarding_completed(&self) {
        let _lock = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let path = self.settings_path();
        let mut value = match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str::<serde_json::Value>(&content).unwrap_or_default(),
            Err(_) => serde_json::Value::Object(serde_json::Map::new()),
        };
        value["onboarding_completed"] = serde_json::Value::Bool(true);
        if let Ok(json) = serde_json::to_string_pretty(&value) {
            let _ = std::fs::write(&path, json);
        }
    }

    // MARK: - Generic Settings

    pub fn load_setting(&self, key: &str) -> Option<String> {
        let path = self.settings_path();
        if !path.exists() { return None; }
        let json = fs::read_to_string(path).ok()?;
        let settings: serde_json::Value = serde_json::from_str(&json).ok()?;
        settings.get(key)?.as_str().map(String::from)
    }

    pub fn save_setting(&self, key: &str, value: &str) {
        let _lock = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let path = self.settings_path();
        let mut settings: serde_json::Value = if path.exists() {
            fs::read_to_string(&path).ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        settings[key] = serde_json::Value::String(value.to_string());
        fs::create_dir_all(path.parent().unwrap()).ok();
        if let Ok(json) = serde_json::to_string(&settings) {
            let _ = fs::write(&path, &json);
        }
    }

    // ─── MiMo Cookie Storage ────────────────────────────────────

    pub fn save_mimo_platform_cookies(&self, cookies: &str) -> io::Result<()> {
        let _lock = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let stored = crate::crypto::encrypt_credential(cookies)
            .map_err(|_e| {
                crate::debug_log!("[storage] mimo_platform_cookies 加密失败，降级为明文: {}", _e);
            })
            .ok()
            .unwrap_or_else(|| cookies.to_string());
        let data = serde_json::json!({
            "CookieHeader": stored,
            "SavedAt": chrono::Utc::now().to_rfc3339()
        });
        let path = self.data_dir().join("mimo_platform_cookies.json");
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, serde_json::to_string(&data)?)?;
        Ok(())
    }

    pub fn load_mimo_platform_cookies(&self) -> Option<String> {
        let path = self.data_dir().join("mimo_platform_cookies.json");
        if !path.exists() { return None; }
        let json = fs::read_to_string(path).ok()?;
        let doc: serde_json::Value = serde_json::from_str(&json).ok()?;
        let stored = doc.get("CookieHeader")?.as_str()?;
        crate::crypto::decrypt_credential(stored).ok()
    }

    pub fn has_saved_mimo_platform_cookies(&self) -> bool {
        self.data_dir().join("mimo_platform_cookies.json").exists()
    }

    pub fn clear_mimo_platform_cookies(&self) {
        let path = self.data_dir().join("mimo_platform_cookies.json");
        fs::remove_file(path).ok();
    }

    pub fn refresh_mimo_platform_cookies_expiry(&self) {
        let path = self.data_dir().join("mimo_platform_cookies.json");
        if !path.exists() { return; }
        let json = match fs::read_to_string(&path) {
            Ok(j) => j,
            Err(_) => return,
        };
        let mut doc: serde_json::Value = match serde_json::from_str(&json) {
            Ok(d) => d,
            Err(_) => return,
        };
        doc["SavedAt"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
        if let Ok(new_json) = serde_json::to_string(&doc) {
            let _ = fs::write(&path, new_json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// 每个测试使用独立临时目录，避免并行执行时互相干扰。
    fn temp_storage(tag: &str) -> Storage {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "modelmeter-test-{}-{}-{}",
            tag,
            std::process::id(),
            n
        ));
        let _ = fs::remove_dir_all(&dir);
        let storage = Storage::new();
        storage.init(dir);
        storage
    }

    #[test]
    fn settings_roundtrip_with_defaults() {
        let s = temp_storage("settings");
        assert_eq!(s.load_refresh_interval(), 60.0);
        s.save_refresh_interval(120.0);
        assert_eq!(s.load_refresh_interval(), 120.0);

        assert!(!s.load_edge_snap_enabled());
        s.save_edge_snap_enabled(true);
        assert!(s.load_edge_snap_enabled());

        assert_eq!(s.load_setting("missing"), None);
        s.save_setting("active_provider", "mimo");
        assert_eq!(s.load_setting("active_provider").as_deref(), Some("mimo"));
    }

    #[test]
    fn onboarding_flag_lifecycle() {
        let s = temp_storage("onboarding");
        assert!(s.is_first_launch());
        s.save_onboarding_completed();
        assert!(!s.is_first_launch());
        assert!(s.load_onboarding_completed());
    }

    #[test]
    fn platform_token_roundtrip_and_clear() {
        let s = temp_storage("token");
        assert!(!s.has_saved_platform_token());
        assert_eq!(s.load_platform_token(), None);

        s.save_platform_token("secret-token-中文").unwrap();
        assert!(s.has_saved_platform_token());
        assert_eq!(s.load_platform_token().as_deref(), Some("secret-token-中文"));

        let raw = fs::read_to_string(s.data_dir().join("platform_token.json")).unwrap();
        #[cfg(target_os = "windows")]
        assert!(raw.contains("enc1:"), "Windows 上凭据应加密存储，实际内容: {}", raw);
        #[cfg(not(target_os = "windows"))]
        assert!(raw.contains("secret-token-中文"));

        s.clear_platform_token();
        assert!(!s.has_saved_platform_token());
    }

    #[test]
    fn platform_cookies_roundtrip() {
        let s = temp_storage("ds-cookies");
        s.save_platform_cookies("session=abc; uid=1").unwrap();
        assert_eq!(s.load_platform_cookies().as_deref(), Some("session=abc; uid=1"));
        s.clear_platform_cookies();
        assert_eq!(s.load_platform_cookies(), None);
    }

    #[test]
    fn mimo_cookies_roundtrip_and_expiry_refresh() {
        let s = temp_storage("mimo");
        assert!(!s.has_saved_mimo_platform_cookies());
        s.save_mimo_platform_cookies("api-platform_ph=abc").unwrap();
        assert!(s.has_saved_mimo_platform_cookies());
        assert_eq!(
            s.load_mimo_platform_cookies().as_deref(),
            Some("api-platform_ph=abc")
        );

        // 续期只更新 SavedAt，不改变已加密内容与可读性
        s.refresh_mimo_platform_cookies_expiry();
        assert_eq!(
            s.load_mimo_platform_cookies().as_deref(),
            Some("api-platform_ph=abc")
        );

        s.clear_mimo_platform_cookies();
        assert!(!s.has_saved_mimo_platform_cookies());
    }

    #[test]
    fn load_decrypts_legacy_plaintext_credentials() {
        // 旧版本（未加密）写入的明文凭据应能被兼容读取
        let s = temp_storage("legacy");
        let path = s.data_dir().join("platform_token.json");
        fs::write(
            &path,
            r#"{"Token":"legacy-plain-token","SavedAt":"2026-01-01T00:00:00Z"}"#,
        )
        .unwrap();
        assert_eq!(
            s.load_platform_token().as_deref(),
            Some("legacy-plain-token")
        );
    }

    #[test]
    fn clear_all_removes_json_files() {
        let s = temp_storage("clear-all");
        s.save_platform_token("t").unwrap();
        s.save_mimo_platform_cookies("c").unwrap();
        s.save_setting("theme", "dark");
        s.clear_all().unwrap();
        assert!(!s.has_saved_platform_token());
        assert!(!s.has_saved_mimo_platform_cookies());
        assert_eq!(s.load_setting("theme"), None);
    }
}
