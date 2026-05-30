pub fn default_device_label() -> String {
    let host = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Vex desktop".to_string());
    format!("Vex Atlas on {host}")
}

pub fn resolve_device_label(input: Option<String>) -> String {
    match input.map(|value| value.trim().to_string()) {
        Some(value) if is_computer_name_placeholder(&value) => default_device_label(),
        Some(value) if !value.is_empty() => value,
        _ => default_device_label(),
    }
}

fn is_computer_name_placeholder(value: &str) -> bool {
    let normalized = value
        .trim_matches('"')
        .trim_matches('\'')
        .to_ascii_uppercase();
    matches!(
        normalized.as_str(),
        "%COMPUTERNAME%"
            | "%COMPUTER%"
            | "$ENV:COMPUTERNAME"
            | "${ENV:COMPUTERNAME}"
            | "$COMPUTERNAME"
            | "${COMPUTERNAME}"
            | "%HOSTNAME%"
            | "$HOSTNAME"
            | "${HOSTNAME}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_explicit_label() {
        assert_eq!(
            resolve_device_label(Some("Bench workstation".to_string())),
            "Bench workstation"
        );
    }

    #[test]
    fn treats_shell_placeholders_as_default_label() {
        let resolved = resolve_device_label(Some("%COMPUTERNAME%".to_string()));
        assert!(resolved.starts_with("Vex Atlas on "));
        assert_ne!(resolved, "%COMPUTERNAME%");
    }
}
