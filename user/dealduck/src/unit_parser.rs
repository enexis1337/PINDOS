#![no_std]
#![feature(alloc)]

use alloc::string::String;
use alloc::vec::Vec;
use alloc::collections::BTreeMap;

/// Parse a systemd unit file
pub fn parse_unit_file(content: &str) -> Result<super::ServiceUnit, ParseError> {
    let mut unit = super::ServiceUnit::new(String::new());
    let mut current_section = SectionType::Unknown;
    
    for (line_num, line) in content.lines().enumerate() {
        // Skip empty lines and comments
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        
        // Check for section header
        if line.starts_with('[') && line.ends_with(']') {
            let section_name = &line[1..line.len()-1];
            current_section = match section_name {
                "Unit" => SectionType::Unit,
                "Service" => SectionType::Service,
                "Install" => SectionType::Install,
                _ => SectionType::Unknown,
            };
            continue;
        }
        
        // Parse key-value pair
        if let Some((key, value)) = split_key_value(line) {
            match current_section {
                SectionType::Unit => parse_unit_key(&mut unit, key, value),
                SectionType::Service => parse_service_key(&mut unit, key, value),
                SectionType::Install => parse_install_key(&mut unit, key, value),
                SectionType::Unknown => {}
            }
        }
    }
    
    Ok(unit)
}

/// Split a line into key and value
fn split_key_value(line: &str) -> Option<(String, String)> {
    let eq_pos = line.find('=')?;
    let key = line[..eq_pos].trim().to_string();
    let value = line[eq_pos+1..].trim().to_string();
    Some((key, value))
}

/// Section types
#[derive(Debug, Clone, Copy, PartialEq)]
enum SectionType {
    Unit,
    Service,
    Install,
    Unknown,
}

/// Parse unit section keys
fn parse_unit_key(unit: &mut super::ServiceUnit, key: String, value: String) {
    match key.as_str() {
        "Description" => unit.unit.description = value,
        "Documentation" => unit.unit.documentation = value,
        "Requires" => unit.unit.requires = split_list(&value),
        "Wants" => unit.unit.wants = split_list(&value),
        "After" => unit.unit.after = split_list(&value),
        "Before" => unit.unit.before = split_list(&value),
        "BindsTo" => unit.unit.binds_to = split_list(&value),
        "PartOf" => unit.unit.part_of = split_list(&value),
        "OnFailure" => unit.unit.on_failure = split_list(&value),
        "OnSuccess" => unit.unit.on_success = split_list(&value),
        "PropagatesReloadTo" => unit.unit.prop_depends = split_list(&value),
        "ReloadPropagatedFrom" => unit.unit.prop_depend_start = split_list(&value),
        "PropagateReloadTo" => unit.unit.prop_depend_stop = split_list(&value),
        "AutomaticallyStart" => unit.unit.prop_automatically = parse_bool(&value),
        "IgnoreOnIsolate" => unit.unit.prop_ignore_on_isolate = parse_bool(&value),
        "AllowIsolate" => unit.unit.prop_allow_isolate = parse_bool(&value),
        "ConditionPathExists" => unit.unit.condition_path_exists = split_list(&value),
        "ConditionPathExistsGlob" => unit.unit.condition_path_exists_glob = split_list(&value),
        "ConditionPathIsMountPoint" => unit.unit.condition_path_is_mount_point = split_list(&value),
        "ConditionPathIsReadOnly" => unit.unit.condition_path_is_read_only = split_list(&value),
        "ConditionDirectoryNotEmpty" => unit.unit.condition_directory_not_empty = split_list(&value),
        "ConditionFileNotEmpty" => unit.unit.condition_file_not_empty = split_list(&value),
        "ConditionFileIsExecutable" => unit.unit.condition_file_is_executable = split_list(&value),
        "ConditionHost" => unit.unit.condition_hostname = value,
        "ConditionVirtualization" => unit.unit.condition_virtualization = value,
        "ConditionArchitecture" => unit.unit.condition_architecture = value,
        "ConditionFirmware" => unit.unit.condition_firmware = value,
        "ConditionKernelCommandLine" => unit.unit.condition_kernel_command_line = split_list(&value),
        "ConditionSecurity" => unit.unit.condition_security = value,
        "ConditionCapability" => unit.unit.condition_capability = split_list(&value),
        "ConditionSystemArchitecture" => unit.unit.condition_system_architecture = value,
        "ConditionSystemVersion" => unit.unit.condition_system_version = value,
        "ConditionSystemRelease" => unit.unit.condition_system_release = value,
        _ => {}
    }
}

/// Parse service section keys
fn parse_service_key(unit: &mut super::ServiceUnit, key: String, value: String) {
    if unit.service.is_none() {
        unit.service = Some(super::ServiceSection::default());
    }
    
    let service = unit.service.as_mut().unwrap();
    
    match key.as_str() {
        "Type" => service.ty = parse_service_type(&value),
        "ExecStart" => service.exec_start = split_list(&value),
        "ExecStartPre" => service.exec_start_pre = split_list(&value),
        "ExecStartPost" => service.exec_start_post = split_list(&value),
        "ExecStop" => service.exec_stop = split_list(&value),
        "ExecStopPost" => service.exec_stop_post = split_list(&value),
        "RestartSec" => service.restart_sec = parse_u64(&value),
        "Restart" => service.restart = parse_restart_policy(&value),
        "TimeoutStartSec" => service.timeout_start_sec = parse_u64(&value),
        "TimeoutStopSec" => service.timeout_stop_sec = parse_u64(&value),
        "TimeoutRestartSec" => service.timeout_restart_sec = parse_u64(&value),
        "Environment" => service.environment = parse_environment(&value),
        "EnvironmentFile" => service.environment_file = split_list(&value),
        "WorkingDirectory" => service.working_directory = value,
        "RootDirectory" => service.root_directory = value,
        "User" => service.user = value,
        "Group" => service.group = value,
        "Nice" => service.nice = parse_i32(&value),
        "UMask" => service.umask = parse_u32(&value),
        "LimitNOFILE" => service.limit_nofile = parse_u64(&value),
        "LimitNPROC" => service.limit_nproc = parse_u64(&value),
        "PrivateTmp" => service.private_tmp = parse_bool(&value),
        "NoNewPrivileges" => service.no_new_privileges = parse_bool(&value),
        "ReadSystemdSecret" => service.read_systemd_secret = parse_bool(&value),
        "SyslogIdentifier" => service.syslog_identifier = value,
        "SyslogLevel" => service.syslog_level = value,
        "SyslogFacility" => service.syslog_facility = value,
        "StandardInput" => service.standard_input = value,
        "StandardOutput" => service.standard_output = value,
        "StandardError" => service.standard_error = value,
        "TTYPath" => service.tty_path = value,
        "TTYReset" => service.tty_reset = parse_bool(&value),
        "TTYVHangup" => service.tty_vhangup = parse_bool(&value),
        "TTYReopen" => service.tty_reopen = parse_bool(&value),
        "IgnoreSIGPIPE" => service.ignore_sigpipe = parse_bool(&value),
        "SendSIGHUP" => service.send_sighup = parse_bool(&value),
        "SendSIGKILL" => service.send_sigkill = parse_bool(&value),
        "WatchdogSec" => service.watchdog_sec = parse_u64(&value),
        _ => {}
    }
}

/// Parse install section keys
fn parse_install_key(unit: &mut super::ServiceUnit, key: String, value: String) {
    if unit.install.is_none() {
        unit.install = Some(super::InstallSection::default());
    }
    
    let install = unit.install.as_mut().unwrap();
    
    match key.as_str() {
        "WantedBy" => install.wanted_by = split_list(&value),
        "RequiredBy" => install.required_by = split_list(&value),
        "Alias" => install.alias = split_list(&value),
        "Also" => install.also = split_list(&value),
        "DefaultInstance" => install.default_instance = value,
        _ => {}
    }
}

/// Split a space-separated list
fn split_list(value: &str) -> Vec<String> {
    value.split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// Parse a boolean value
fn parse_bool(value: &str) -> bool {
    match value.to_lowercase().as_str() {
        "true" | "yes" | "1" | "on" => true,
        "false" | "no" | "0" | "off" => false,
        _ => false,
    }
}

/// Parse a u64 value
fn parse_u64(value: &str) -> u64 {
    value.parse().unwrap_or(0)
}

/// Parse a i32 value
fn parse_i32(value: &str) -> i32 {
    value.parse().unwrap_or(0)
}

/// Parse a u32 value
fn parse_u32(value: &str) -> u32 {
    value.parse().unwrap_or(0)
}

/// Parse service type
fn parse_service_type(value: &str) -> super::ServiceType {
    match value.to_lowercase().as_str() {
        "simple" => super::ServiceType::Simple,
        "forking" => super::ServiceType::Forking,
        "oneshot" => super::ServiceType::Oneshot,
        "dbus" => super::ServiceType::Dbus,
        "notify" => super::ServiceType::Notify,
        "idle" => super::ServiceType::Idle,
        _ => super::ServiceType::Unknown,
    }
}

/// Parse restart policy
fn parse_restart_policy(value: &str) -> super::RestartPolicy {
    match value.to_lowercase().as_str() {
        "no" => super::RestartPolicy::No,
        "on-success" => super::RestartPolicy::OnSuccess,
        "on-failure" => super::RestartPolicy::OnFailure,
        "on-abnormal" => super::RestartPolicy::OnAbnormal,
        "on-watchdog" => super::RestartPolicy::OnWatchdog,
        "always" => super::RestartPolicy::Always,
        "on-success-or-abnormal" => super::RestartPolicy::OnSuccessOrAbnormal,
        "on-success-or-failure" => super::RestartPolicy::OnSuccessOrFailure,
        _ => super::RestartPolicy::Unknown,
    }
}

/// Parse environment variables
fn parse_environment(value: &str) -> Vec<(String, String)> {
    let mut env = Vec::new();
    for part in value.split_whitespace() {
        if let Some(eq_pos) = part.find('=') {
            let key = part[..eq_pos].to_string();
            let val = part[eq_pos+1..].to_string();
            env.push((key, val));
        }
    }
    env
}

/// Parse error type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ParseError {
    InvalidFormat,
    MissingSection,
    InvalidValue,
    Unknown,
}