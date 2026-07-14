use dot_providers::ChatConfig;

fn config_with(base_url: &str) -> ChatConfig {
    ChatConfig {
        base_url: base_url.parse().unwrap(),
        api_key: String::new(),
        model: String::new(),
    }
}

#[test]
fn loopback_addresses_stay_on_this_mac() {
    for base_url in [
        "http://127.0.0.1:8080/v1",
        "http://localhost:11434/v1",
        "http://[::1]:1234/v1",
    ] {
        assert!(!config_with(base_url).leaves_device(), "{base_url}");
    }
}

#[test]
fn other_hosts_leave_this_mac() {
    for base_url in [
        "https://api.openai.com/v1",
        "http://192.168.1.20:8080/v1",
        "http://localhost.example.com/v1",
    ] {
        assert!(config_with(base_url).leaves_device(), "{base_url}");
    }
}
