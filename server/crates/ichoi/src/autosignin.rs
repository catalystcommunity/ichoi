//! Windows auto sign-in for headless satellites.
//!
//! Windows 11 renders audio from services only while a user is signed in. A locked session is
//! enough. A headless satellite therefore signs in a dedicated local account at boot and locks
//! it at once. The password is random, never shown, and stored only as the LSA secret that
//! Winlogon reads.
//!
//! The preflight rules are pure so they are tested on every host. Reading and changing the
//! machine is Windows-only.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The dedicated local account that signs in at boot.
pub const ACCOUNT: &str = "IchoiAudio";
/// The scheduled task that locks the account's session when it signs in.
pub const LOCK_TASK: &str = "Ichoi Auto-Signin Lock";

/// The parts of the machine's sign-in configuration that auto sign-in depends on.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct HostState {
    pub auto_admin_logon: Option<String>,
    pub default_user_name: Option<String>,
    pub default_domain_name: Option<String>,
    /// A plaintext `DefaultPassword` registry value, which Winlogon prefers over the secret.
    pub plaintext_password: bool,
    /// `AutoLogonCount` limits auto sign-in to a number of boots.
    pub auto_logon_count: Option<u32>,
    /// `DevicePasswordLessBuildVersion`: non-zero means Windows Hello "passwordless" sign-in,
    /// which turns auto sign-in off.
    pub passwordless: Option<u32>,
    pub disable_privacy_experience: Option<u32>,
    pub first_logon_animation: Option<u32>,
    pub legal_notice_caption: Option<String>,
    pub legal_notice_text: Option<String>,
    pub hide_fast_user_switching: Option<u32>,
    pub domain_joined: bool,
    pub account_exists: bool,
    /// Ichoi created the current configuration (its undo file exists).
    pub managed_by_ichoi: bool,
}

/// What auto sign-in will change, what stops it, and what the owner should know.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Preflight {
    pub blockers: Vec<String>,
    pub changes: Vec<String>,
    pub warnings: Vec<String>,
}

fn is_set(value: &Option<String>) -> bool {
    value.as_deref().is_some_and(|v| !v.trim().is_empty())
}

fn is_account(name: &Option<String>) -> bool {
    name.as_deref()
        .is_some_and(|n| n.trim().eq_ignore_ascii_case(ACCOUNT))
}

/// `replace` allows taking over another account's automatic sign-in or an existing account
/// that Ichoi did not create.
pub fn preflight(host: &HostState, replace: bool) -> Preflight {
    let mut p = Preflight::default();
    let auto_logon_on = host.auto_admin_logon.as_deref().map(str::trim) == Some("1");

    if auto_logon_on && !is_account(&host.default_user_name) && !replace {
        p.blockers.push(format!(
            "Windows already signs in {} automatically. Only one account can sign in automatically; use --replace-auto-signin to switch it to {ACCOUNT}",
            host.default_user_name.as_deref().unwrap_or("another account")
        ));
    }
    if is_set(&host.legal_notice_caption) || is_set(&host.legal_notice_text) {
        p.blockers.push(
            "a sign-in legal notice is configured; Windows waits for someone to accept it, so auto sign-in cannot finish"
                .into(),
        );
    }
    if host.account_exists && !host.managed_by_ichoi && !replace {
        p.blockers.push(format!(
            "a local account named {ACCOUNT} already exists and was not created by Ichoi; use --replace-auto-signin to reset its password and use it"
        ));
    }

    if host.account_exists {
        p.changes
            .push(format!("reset the password of local account {ACCOUNT}"));
    } else {
        p.changes.push(format!(
            "create standard local account {ACCOUNT} (password never expires, hidden from the sign-in screen)"
        ));
    }
    p.changes.push(
        "store a random password as the Winlogon LSA secret (it is never shown or written to a file)"
            .into(),
    );
    p.changes
        .push(format!("sign in {ACCOUNT} automatically at every boot"));
    p.changes.push(format!(
        "add scheduled task \"{LOCK_TASK}\" that locks the {ACCOUNT} session as soon as it signs in"
    ));
    if let Some(version) = host.passwordless.filter(|v| *v != 0) {
        p.changes.push(format!(
            "turn off Windows Hello passwordless-only sign-in (DevicePasswordLessBuildVersion {version} -> 0); the sign-in screen also offers password sign-in afterwards"
        ));
    }
    if host.plaintext_password {
        p.changes.push(
            "remove the plaintext Winlogon DefaultPassword registry value, which would override the secret"
                .into(),
        );
    }
    if host.auto_logon_count.is_some() {
        p.changes.push(
            "remove Winlogon AutoLogonCount, which would stop auto sign-in after a number of boots"
                .into(),
        );
    }
    if host.disable_privacy_experience != Some(1) {
        p.changes.push(
            "skip the first sign-in privacy settings screen (DisablePrivacyExperience = 1)".into(),
        );
    }
    if host.first_logon_animation != Some(0) {
        p.changes
            .push("skip the first sign-in animation (EnableFirstLogonAnimation = 0)".into());
    }

    if host.hide_fast_user_switching == Some(1) {
        p.warnings.push(format!(
            "fast user switching is turned off by policy: to use this PC you must first sign out {ACCOUNT}, which stops audio until the next restart"
        ));
    }
    if host.domain_joined {
        p.warnings.push(
            "this PC is joined to a domain; domain Group Policy can overwrite the auto sign-in settings"
                .into(),
        );
    }
    p.warnings.push(format!(
        "anyone who restarts this PC gets a signed-in {ACCOUNT} session for a moment before it locks, and any administrator can read the stored password"
    ));
    p
}

pub fn describe(p: &Preflight) -> String {
    let mut out = String::from(
        "auto sign-in (Windows 11 plays service audio only while a user is signed in)\n",
    );
    for change in &p.changes {
        let _ = writeln!(out, "  change: {change}");
    }
    for warning in &p.warnings {
        let _ = writeln!(out, "  warning: {warning}");
    }
    for blocker in &p.blockers {
        let _ = writeln!(out, "  blocked: {blocker}");
    }
    out
}

/// The undo file for auto sign-in, beside the satellite configuration.
pub fn state_path(config: &Path) -> PathBuf {
    config.with_file_name("autosignin.json")
}

/// Values that existed before Ichoi changed them, so uninstall can restore them.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Previous {
    created_account: bool,
    auto_admin_logon: Option<String>,
    default_user_name: Option<String>,
    default_domain_name: Option<String>,
    auto_logon_count: Option<u32>,
    passwordless: Option<u32>,
    disable_privacy_experience: Option<u32>,
    first_logon_animation: Option<u32>,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
impl Previous {
    fn from_host(host: &HostState) -> Previous {
        Previous {
            created_account: !host.account_exists,
            auto_admin_logon: host.auto_admin_logon.clone(),
            default_user_name: host.default_user_name.clone(),
            default_domain_name: host.default_domain_name.clone(),
            auto_logon_count: host.auto_logon_count,
            passwordless: host.passwordless,
            disable_privacy_experience: host.disable_privacy_experience,
            first_logon_animation: host.first_logon_animation,
        }
    }

    /// Whether another account signed in automatically before Ichoi replaced it.
    fn replaced_other_account(&self) -> Option<&str> {
        (self.auto_admin_logon.as_deref().map(str::trim) == Some("1"))
            .then_some(self.default_user_name.as_deref())
            .flatten()
            .filter(|name| !name.trim().eq_ignore_ascii_case(ACCOUNT))
    }
}

/// Task Scheduler XML for the lock task, limited to the account's own sign-in.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn lock_task_xml(computer: &str) -> String {
    let user = format!("{computer}\\{ACCOUNT}");
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Locks the Ichoi audio session as soon as it signs in at boot.</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <ExecutionTimeLimit>PT1M</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>rundll32.exe</Command>
      <Arguments>user32.dll,LockWorkStation</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

#[cfg(target_os = "windows")]
pub use windows::{disable, enable, read_host_state, status};

#[cfg(not(target_os = "windows"))]
pub fn read_host_state(_config: &Path) -> anyhow::Result<HostState> {
    anyhow::bail!("auto sign-in is available only on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn enable(_config: &Path, _host: &HostState) -> anyhow::Result<()> {
    anyhow::bail!("auto sign-in is available only on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn disable(_config: &Path) -> anyhow::Result<bool> {
    Ok(false)
}

#[cfg(not(target_os = "windows"))]
pub fn status(_config: &Path) -> String {
    "auto sign-in: not available on this platform".into()
}

#[cfg(target_os = "windows")]
mod windows {
    use std::path::Path;
    use std::process::Command;
    use std::ptr;

    use rand::RngCore;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::NetworkManagement::NetManagement::{
        NERR_Success, NERR_UserNotFound, NetApiBufferFree, NetGetJoinInformation,
        NetSetupDomainName, NetUserAdd, NetUserDel, NetUserGetInfo, NetUserSetInfo,
        UF_DONT_EXPIRE_PASSWD, UF_PASSWD_CANT_CHANGE, UF_SCRIPT, USER_INFO_1, USER_INFO_1003,
        USER_INFO_1008, USER_PRIV_USER,
    };
    use windows_sys::Win32::Security::Authentication::Identity::{
        LsaClose, LsaNtStatusToWinError, LsaOpenPolicy, LsaStorePrivateData, LSA_HANDLE,
        LSA_OBJECT_ATTRIBUTES, LSA_UNICODE_STRING, POLICY_CREATE_SECRET,
    };
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW, HKEY_LOCAL_MACHINE, REG_DWORD, REG_SZ,
        RRF_RT_ANY, RRF_RT_REG_DWORD, RRF_RT_REG_SZ,
    };
    use windows_sys::Win32::System::RemoteDesktop::{
        WTSEnumerateSessionsW, WTSFreeMemory, WTSLogoffSession, WTSQuerySessionInformationW,
        WTSUserName, WTS_CURRENT_SERVER_HANDLE, WTS_SESSION_INFOW,
    };

    use super::{lock_task_xml, state_path, HostState, Previous, ACCOUNT, LOCK_TASK};

    const WINLOGON: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon";
    const HIDDEN: &str =
        r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon\SpecialAccounts\UserList";
    const PASSWORDLESS: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\PasswordLess\Device";
    const OOBE: &str = r"SOFTWARE\Policies\Microsoft\Windows\OOBE";
    const SYSTEM: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System";
    const ACCOUNT_COMMENT: &str = "Ichoi: signs in at boot so service audio plays";

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn reg_get_string(key: &str, name: &str) -> Option<String> {
        let (key, name) = (wide(key), wide(name));
        let mut size = 0u32;
        // SAFETY: valid NUL-terminated strings; a null data pointer asks only for the size.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        // SAFETY: `buffer` holds `size` bytes.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let len = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
        Some(String::from_utf16_lossy(&buffer[..len]))
    }

    fn reg_get_dword(key: &str, name: &str) -> Option<u32> {
        let (key, name) = (wide(key), wide(name));
        let mut value = 0u32;
        let mut size = 4u32;
        // SAFETY: `value` holds the 4 bytes a REG_DWORD needs.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_DWORD,
                ptr::null_mut(),
                (&mut value as *mut u32).cast(),
                &mut size,
            )
        };
        (status == ERROR_SUCCESS).then_some(value)
    }

    fn reg_exists(key: &str, name: &str) -> bool {
        let (key, name) = (wide(key), wide(name));
        let mut size = 0u32;
        // SAFETY: a null data pointer asks only whether the value exists and its size.
        unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_ANY,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
            ) == ERROR_SUCCESS
        }
    }

    fn reg_set_string(key: &str, name: &str, value: &str) -> anyhow::Result<()> {
        let (k, n, v) = (wide(key), wide(name), wide(value));
        // SAFETY: `v` is a NUL-terminated UTF-16 string of the given byte length.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_LOCAL_MACHINE,
                k.as_ptr(),
                n.as_ptr(),
                REG_SZ,
                v.as_ptr().cast(),
                (v.len() * 2) as u32,
            )
        };
        check(status, || format!(r"setting HKLM\{key}\{name}"))
    }

    fn reg_set_dword(key: &str, name: &str, value: u32) -> anyhow::Result<()> {
        let (k, n) = (wide(key), wide(name));
        // SAFETY: `value` is a 4-byte REG_DWORD.
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_LOCAL_MACHINE,
                k.as_ptr(),
                n.as_ptr(),
                REG_DWORD,
                (&value as *const u32).cast(),
                4,
            )
        };
        check(status, || format!(r"setting HKLM\{key}\{name}"))
    }

    fn reg_delete(key: &str, name: &str) -> anyhow::Result<()> {
        let (k, n) = (wide(key), wide(name));
        // SAFETY: valid NUL-terminated strings.
        let status = unsafe { RegDeleteKeyValueW(HKEY_LOCAL_MACHINE, k.as_ptr(), n.as_ptr()) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(());
        }
        check(status, || format!(r"deleting HKLM\{key}\{name}"))
    }

    fn reg_restore_string(key: &str, name: &str, previous: Option<&str>) -> anyhow::Result<()> {
        match previous {
            Some(value) => reg_set_string(key, name, value),
            None => reg_delete(key, name),
        }
    }

    fn reg_restore_dword(key: &str, name: &str, previous: Option<u32>) -> anyhow::Result<()> {
        match previous {
            Some(value) => reg_set_dword(key, name, value),
            None => reg_delete(key, name),
        }
    }

    fn check(status: u32, what: impl FnOnce() -> String) -> anyhow::Result<()> {
        if status == ERROR_SUCCESS {
            return Ok(());
        }
        Err(anyhow::anyhow!(
            "{}: {}",
            what(),
            std::io::Error::from_raw_os_error(status as i32)
        ))
    }

    fn account_exists() -> bool {
        let name = wide(ACCOUNT);
        let mut buffer = ptr::null_mut();
        // SAFETY: level 0 returns a buffer that is freed below.
        let status = unsafe { NetUserGetInfo(ptr::null(), name.as_ptr(), 0, &mut buffer) };
        if !buffer.is_null() {
            // SAFETY: allocated by NetUserGetInfo.
            unsafe { NetApiBufferFree(buffer.cast()) };
        }
        status == NERR_Success
    }

    fn domain_joined() -> bool {
        let mut name = ptr::null_mut();
        let mut kind = 0;
        // SAFETY: both out pointers are valid; the name buffer is freed below.
        let status = unsafe { NetGetJoinInformation(ptr::null(), &mut name, &mut kind) };
        if !name.is_null() {
            // SAFETY: allocated by NetGetJoinInformation.
            unsafe { NetApiBufferFree(name.cast()) };
        }
        status == NERR_Success && kind == NetSetupDomainName
    }

    pub fn read_host_state(config: &Path) -> anyhow::Result<HostState> {
        Ok(HostState {
            auto_admin_logon: reg_get_string(WINLOGON, "AutoAdminLogon"),
            default_user_name: reg_get_string(WINLOGON, "DefaultUserName"),
            default_domain_name: reg_get_string(WINLOGON, "DefaultDomainName"),
            plaintext_password: reg_exists(WINLOGON, "DefaultPassword"),
            auto_logon_count: reg_get_dword(WINLOGON, "AutoLogonCount"),
            passwordless: reg_get_dword(PASSWORDLESS, "DevicePasswordLessBuildVersion"),
            disable_privacy_experience: reg_get_dword(OOBE, "DisablePrivacyExperience"),
            first_logon_animation: reg_get_dword(SYSTEM, "EnableFirstLogonAnimation"),
            legal_notice_caption: reg_get_string(SYSTEM, "legalnoticecaption"),
            legal_notice_text: reg_get_string(SYSTEM, "legalnoticetext"),
            hide_fast_user_switching: reg_get_dword(SYSTEM, "HideFastUserSwitching"),
            domain_joined: domain_joined(),
            account_exists: account_exists(),
            managed_by_ichoi: state_path(config).exists(),
        })
    }

    fn random_password() -> String {
        let mut bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut bytes);
        // Hex plus one character of each other class satisfies any complexity policy.
        format!("{}Aa1!", hex::encode(bytes))
    }

    fn net_status(status: u32, what: &str) -> anyhow::Result<()> {
        if status == NERR_Success {
            return Ok(());
        }
        Err(anyhow::anyhow!(
            "{what}: {}",
            std::io::Error::from_raw_os_error(status as i32)
        ))
    }

    fn create_account(password: &str) -> anyhow::Result<()> {
        let mut name = wide(ACCOUNT);
        let mut pass = wide(password);
        let mut comment = wide(ACCOUNT_COMMENT);
        let info = USER_INFO_1 {
            usri1_name: name.as_mut_ptr(),
            usri1_password: pass.as_mut_ptr(),
            usri1_password_age: 0,
            usri1_priv: USER_PRIV_USER,
            usri1_home_dir: ptr::null_mut(),
            usri1_comment: comment.as_mut_ptr(),
            usri1_flags: UF_SCRIPT | UF_DONT_EXPIRE_PASSWD | UF_PASSWD_CANT_CHANGE,
            usri1_script_path: ptr::null_mut(),
        };
        let mut parm = 0u32;
        // SAFETY: `info` points at NUL-terminated strings that outlive the call.
        let status = unsafe {
            NetUserAdd(
                ptr::null(),
                1,
                (&info as *const USER_INFO_1).cast(),
                &mut parm,
            )
        };
        pass.fill(0);
        net_status(status, &format!("creating local account {ACCOUNT}"))
    }

    fn reset_account(password: &str) -> anyhow::Result<()> {
        let name = wide(ACCOUNT);
        let mut pass = wide(password);
        let info = USER_INFO_1003 {
            usri1003_password: pass.as_mut_ptr(),
        };
        let mut parm = 0u32;
        // SAFETY: `info` points at a NUL-terminated string that outlives the call.
        let status = unsafe {
            NetUserSetInfo(
                ptr::null(),
                name.as_ptr(),
                1003,
                (&info as *const USER_INFO_1003).cast(),
                &mut parm,
            )
        };
        pass.fill(0);
        net_status(status, &format!("resetting the password of {ACCOUNT}"))?;
        let flags = USER_INFO_1008 {
            usri1008_flags: UF_SCRIPT | UF_DONT_EXPIRE_PASSWD | UF_PASSWD_CANT_CHANGE,
        };
        // SAFETY: `flags` is a valid USER_INFO_1008.
        let status = unsafe {
            NetUserSetInfo(
                ptr::null(),
                name.as_ptr(),
                1008,
                (&flags as *const USER_INFO_1008).cast(),
                &mut parm,
            )
        };
        net_status(status, &format!("setting account flags of {ACCOUNT}"))
    }

    /// Store (or, with `None`, delete) the LSA secret that Winlogon reads the password from.
    fn store_secret(value: Option<&str>) -> anyhow::Result<()> {
        let attributes = LSA_OBJECT_ATTRIBUTES {
            Length: std::mem::size_of::<LSA_OBJECT_ATTRIBUTES>() as u32,
            RootDirectory: ptr::null_mut(),
            ObjectName: ptr::null_mut(),
            Attributes: 0,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
        };
        let mut policy: LSA_HANDLE = 0;
        // SAFETY: valid attributes and out handle; a null system name means this computer.
        let status = unsafe {
            LsaOpenPolicy(
                ptr::null(),
                &attributes,
                POLICY_CREATE_SECRET as u32,
                &mut policy,
            )
        };
        if status != 0 {
            // SAFETY: plain status conversion.
            let error = unsafe { LsaNtStatusToWinError(status) };
            anyhow::bail!(
                "opening the LSA policy: {}",
                std::io::Error::from_raw_os_error(error as i32)
            );
        }
        let mut key = wide("DefaultPassword");
        let key_string = LSA_UNICODE_STRING {
            Length: ((key.len() - 1) * 2) as u16,
            MaximumLength: (key.len() * 2) as u16,
            Buffer: key.as_mut_ptr(),
        };
        let mut data = value.map(wide);
        let data_string = data.as_mut().map(|d| LSA_UNICODE_STRING {
            Length: ((d.len() - 1) * 2) as u16,
            MaximumLength: (d.len() * 2) as u16,
            Buffer: d.as_mut_ptr(),
        });
        let data_ptr = data_string
            .as_ref()
            .map_or(ptr::null(), |d| d as *const LSA_UNICODE_STRING);
        // SAFETY: the strings outlive the call; a null data pointer deletes the secret.
        let status = unsafe { LsaStorePrivateData(policy, &key_string, data_ptr) };
        if let Some(d) = data.as_mut() {
            d.fill(0);
        }
        // SAFETY: `policy` was opened above.
        unsafe { LsaClose(policy) };
        // STATUS_OBJECT_NAME_NOT_FOUND when deleting a secret that does not exist.
        if status == 0 || (value.is_none() && status as u32 == 0xC000_0034) {
            return Ok(());
        }
        // SAFETY: plain status conversion.
        let error = unsafe { LsaNtStatusToWinError(status) };
        anyhow::bail!(
            "storing the auto sign-in secret: {}",
            std::io::Error::from_raw_os_error(error as i32)
        )
    }

    fn run(program: &str, args: &[&str]) -> anyhow::Result<()> {
        let output = Command::new(program).args(args).output()?;
        if !output.status.success() {
            anyhow::bail!(
                "{program} {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(())
    }

    fn install_lock_task(config: &Path, computer: &str) -> anyhow::Result<()> {
        // Task Scheduler reads XML as UTF-16 with a byte order mark.
        let xml = lock_task_xml(computer);
        let mut bytes = vec![0xFF, 0xFE];
        for unit in xml.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let path = config.with_file_name("autosignin-lock-task.xml");
        std::fs::write(&path, bytes)?;
        let result = run(
            "schtasks.exe",
            &[
                "/Create",
                "/F",
                "/TN",
                LOCK_TASK,
                "/XML",
                &path.display().to_string(),
            ],
        );
        let _ = std::fs::remove_file(&path);
        result
    }

    pub fn enable(config: &Path, host: &HostState) -> anyhow::Result<()> {
        let state = state_path(config);
        // Keep the values from before the first installation when reinstalling.
        if !state.exists() {
            let previous = Previous::from_host(host);
            if let Some(parent) = state.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&state, serde_json::to_vec_pretty(&previous)?)?;
        }
        let computer = std::env::var("COMPUTERNAME")
            .map_err(|_| anyhow::anyhow!("COMPUTERNAME is not set"))?;

        let mut password = random_password();
        let account = if host.account_exists {
            reset_account(&password)
        } else {
            create_account(&password)
        };
        let secret = account.and_then(|()| store_secret(Some(&password)));
        // SAFETY: overwriting ASCII with zero bytes keeps the string valid UTF-8.
        unsafe { password.as_bytes_mut().fill(0) };
        secret?;

        reg_set_string(WINLOGON, "AutoAdminLogon", "1")?;
        reg_set_string(WINLOGON, "DefaultUserName", ACCOUNT)?;
        reg_set_string(WINLOGON, "DefaultDomainName", &computer)?;
        reg_delete(WINLOGON, "DefaultPassword")?;
        reg_delete(WINLOGON, "AutoLogonCount")?;
        reg_set_dword(PASSWORDLESS, "DevicePasswordLessBuildVersion", 0)?;
        reg_set_dword(OOBE, "DisablePrivacyExperience", 1)?;
        reg_set_dword(SYSTEM, "EnableFirstLogonAnimation", 0)?;
        reg_set_dword(HIDDEN, ACCOUNT, 0)?;
        install_lock_task(config, &computer)?;
        Ok(())
    }

    /// Session ids that belong to the auto sign-in account.
    fn account_sessions() -> Vec<u32> {
        let mut sessions: *mut WTS_SESSION_INFOW = ptr::null_mut();
        let mut count = 0u32;
        // SAFETY: out pointers are valid; the list is freed below.
        let ok = unsafe {
            WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &mut sessions, &mut count)
        };
        if ok == 0 || sessions.is_null() {
            return Vec::new();
        }
        // SAFETY: WTSEnumerateSessionsW returned `count` entries.
        let list = unsafe { std::slice::from_raw_parts(sessions, count as usize) };
        let mut ids = Vec::new();
        for session in list {
            let mut name = ptr::null_mut();
            let mut bytes = 0u32;
            // SAFETY: valid out pointers; the name buffer is freed below.
            let ok = unsafe {
                WTSQuerySessionInformationW(
                    WTS_CURRENT_SERVER_HANDLE,
                    session.SessionId,
                    WTSUserName,
                    &mut name,
                    &mut bytes,
                )
            };
            if ok == 0 || name.is_null() {
                continue;
            }
            // SAFETY: WTS returned a NUL-terminated string of `bytes` bytes.
            let user = unsafe {
                let units = std::slice::from_raw_parts(name, (bytes as usize) / 2);
                let len = units.iter().position(|c| *c == 0).unwrap_or(units.len());
                String::from_utf16_lossy(&units[..len])
            };
            // SAFETY: allocated by WTSQuerySessionInformationW.
            unsafe { WTSFreeMemory(name.cast()) };
            if user.eq_ignore_ascii_case(ACCOUNT) {
                ids.push(session.SessionId);
            }
        }
        // SAFETY: allocated by WTSEnumerateSessionsW.
        unsafe { WTSFreeMemory(sessions.cast()) };
        ids
    }

    /// Sign out the account and wait until its sessions are gone. A profile that is still
    /// loaded cannot be deleted, and deleting the account first leaves an orphaned session.
    fn sign_out_account() -> anyhow::Result<()> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            let sessions = account_sessions();
            if sessions.is_empty() {
                return Ok(());
            }
            if std::time::Instant::now() > deadline {
                anyhow::bail!(
                    "{ACCOUNT} is still signed in (sessions {sessions:?}); sign it out or restart Windows, then run uninstall again"
                );
            }
            for id in sessions {
                // SAFETY: plain call with a session id from the enumeration.
                unsafe { WTSLogoffSession(WTS_CURRENT_SERVER_HANDLE, id, 1) };
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    /// Delete the account's profile folder and registry entry. Windows unloads a profile
    /// shortly after sign-out, so retry for a while.
    fn remove_profile() {
        let script = format!(
            "$ErrorActionPreference='Stop'; for ($i = 0; $i -lt 15; $i++) {{ $p = Get-CimInstance Win32_UserProfile | Where-Object {{ $_.LocalPath -match '\\\\{ACCOUNT}(\\.[^\\\\]+)?$' }}; if (-not $p) {{ exit 0 }}; try {{ $p | Remove-CimInstance; exit 0 }} catch {{ Start-Sleep 2 }} }}; exit 1"
        );
        let removed = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
            .is_ok_and(|o| o.status.success());
        if !removed {
            println!(
                "  note: could not remove the {ACCOUNT} profile folder; remove it in Settings > Accounts > Other users after a restart"
            );
        }
    }

    /// Remove auto sign-in and restore the values recorded at installation. Returns whether
    /// anything was configured.
    pub fn disable(config: &Path) -> anyhow::Result<bool> {
        let state = state_path(config);
        let previous: Option<Previous> = std::fs::read(&state)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let configured_for_account = reg_get_string(WINLOGON, "DefaultUserName")
            .is_some_and(|name| name.eq_ignore_ascii_case(ACCOUNT));
        if previous.is_none() && !configured_for_account {
            return Ok(false);
        }
        let previous = previous.unwrap_or_default();

        let _ = run("schtasks.exe", &["/Delete", "/F", "/TN", LOCK_TASK]);
        store_secret(None)?;
        if let Some(other) = previous.replaced_other_account() {
            // The other account's password was replaced, so it cannot sign in automatically
            // again until someone enters it with a tool such as Sysinternals Autologon.
            println!(
                "  note: {other} signed in automatically before Ichoi; set its password again to restore that"
            );
            reg_set_string(WINLOGON, "AutoAdminLogon", "0")?;
            reg_set_string(WINLOGON, "DefaultUserName", other)?;
            if let Some(domain) = &previous.default_domain_name {
                reg_set_string(WINLOGON, "DefaultDomainName", domain)?;
            }
        } else {
            // Restoring "1" here would point Winlogon at an account that no longer exists.
            let auto = previous
                .auto_admin_logon
                .as_deref()
                .filter(|v| v.trim() != "1");
            reg_restore_string(WINLOGON, "AutoAdminLogon", auto)?;
            let user = previous
                .default_user_name
                .as_deref()
                .filter(|name| !name.eq_ignore_ascii_case(ACCOUNT));
            reg_restore_string(WINLOGON, "DefaultUserName", user)?;
            reg_restore_string(
                WINLOGON,
                "DefaultDomainName",
                previous.default_domain_name.as_deref(),
            )?;
        }
        if let Some(count) = previous.auto_logon_count {
            reg_set_dword(WINLOGON, "AutoLogonCount", count)?;
        }
        reg_restore_dword(
            PASSWORDLESS,
            "DevicePasswordLessBuildVersion",
            previous.passwordless,
        )?;
        reg_restore_dword(
            OOBE,
            "DisablePrivacyExperience",
            previous.disable_privacy_experience,
        )?;
        reg_restore_dword(
            SYSTEM,
            "EnableFirstLogonAnimation",
            previous.first_logon_animation,
        )?;
        reg_delete(HIDDEN, ACCOUNT)?;

        if previous.created_account {
            sign_out_account()?;
            remove_profile();
            let name = wide(ACCOUNT);
            // SAFETY: valid NUL-terminated string.
            let status = unsafe { NetUserDel(ptr::null(), name.as_ptr()) };
            if status != NERR_UserNotFound {
                net_status(status, &format!("deleting local account {ACCOUNT}"))?;
            }
        }
        let _ = std::fs::remove_file(&state);
        Ok(true)
    }

    pub fn status(config: &Path) -> String {
        let on = reg_get_string(WINLOGON, "AutoAdminLogon").as_deref() == Some("1")
            && reg_get_string(WINLOGON, "DefaultUserName")
                .is_some_and(|name| name.eq_ignore_ascii_case(ACCOUNT));
        let task = Command::new("schtasks.exe")
            .args(["/Query", "/TN", LOCK_TASK])
            .output()
            .is_ok_and(|o| o.status.success());
        match (on, state_path(config).exists()) {
            (true, _) => format!(
                "auto sign-in: enabled for {ACCOUNT} (lock task {})",
                if task { "present" } else { "MISSING" }
            ),
            (false, true) => format!(
                "auto sign-in: configured by Ichoi but Winlogon no longer signs in {ACCOUNT}; something changed the setting"
            ),
            (false, false) => "auto sign-in: not configured".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_host() -> HostState {
        HostState {
            passwordless: Some(2),
            ..HostState::default()
        }
    }

    #[test]
    fn clean_home_pc_has_no_blockers_and_disables_passwordless() {
        let p = preflight(&clean_host(), false);
        assert!(p.blockers.is_empty(), "{:?}", p.blockers);
        assert!(p
            .changes
            .iter()
            .any(|c| c.contains("DevicePasswordLessBuildVersion 2 -> 0")));
        assert!(p
            .changes
            .iter()
            .any(|c| c.contains("create standard local account")));
    }

    #[test]
    fn another_accounts_auto_sign_in_needs_replace() {
        let host = HostState {
            auto_admin_logon: Some("1".into()),
            default_user_name: Some("owner".into()),
            ..clean_host()
        };
        assert_eq!(preflight(&host, false).blockers.len(), 1);
        assert!(preflight(&host, true).blockers.is_empty());
    }

    #[test]
    fn existing_ichoi_auto_sign_in_is_not_a_conflict() {
        let host = HostState {
            auto_admin_logon: Some("1".into()),
            default_user_name: Some("ichoiaudio".into()),
            account_exists: true,
            managed_by_ichoi: true,
            ..clean_host()
        };
        let p = preflight(&host, false);
        assert!(p.blockers.is_empty(), "{:?}", p.blockers);
        assert!(p.changes.iter().any(|c| c.contains("reset the password")));
    }

    #[test]
    fn blank_legal_notice_is_ignored_but_real_text_blocks() {
        let blank = HostState {
            legal_notice_text: Some(" ".into()),
            ..clean_host()
        };
        assert!(preflight(&blank, false).blockers.is_empty());
        let notice = HostState {
            legal_notice_caption: Some("Authorized use only".into()),
            ..clean_host()
        };
        assert_eq!(preflight(&notice, true).blockers.len(), 1);
    }

    #[test]
    fn unrelated_existing_account_needs_replace() {
        let host = HostState {
            account_exists: true,
            ..clean_host()
        };
        assert_eq!(preflight(&host, false).blockers.len(), 1);
        assert!(preflight(&host, true).blockers.is_empty());
    }

    #[test]
    fn policies_that_break_auto_sign_in_are_changed_or_warned() {
        let host = HostState {
            plaintext_password: true,
            auto_logon_count: Some(3),
            hide_fast_user_switching: Some(1),
            domain_joined: true,
            ..clean_host()
        };
        let p = preflight(&host, false);
        assert!(p.changes.iter().any(|c| c.contains("plaintext")));
        assert!(p.changes.iter().any(|c| c.contains("AutoLogonCount")));
        assert!(p.warnings.iter().any(|w| w.contains("fast user switching")));
        assert!(p.warnings.iter().any(|w| w.contains("domain")));
    }

    #[test]
    fn previous_state_remembers_another_auto_sign_in_account() {
        let host = HostState {
            auto_admin_logon: Some("1".into()),
            default_user_name: Some("owner".into()),
            ..clean_host()
        };
        let previous = Previous::from_host(&host);
        assert_eq!(previous.replaced_other_account(), Some("owner"));
        assert!(previous.created_account);
        let ours = Previous::from_host(&HostState {
            auto_admin_logon: Some("1".into()),
            default_user_name: Some(ACCOUNT.into()),
            ..clean_host()
        });
        assert_eq!(ours.replaced_other_account(), None);
    }

    #[test]
    fn lock_task_triggers_only_for_the_account() {
        let xml = lock_task_xml("HT0-BASESPEAKER");
        assert_eq!(xml.matches(r"HT0-BASESPEAKER\IchoiAudio").count(), 2);
        assert!(xml.contains("LockWorkStation"));
    }
}
