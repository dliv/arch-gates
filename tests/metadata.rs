use arch_gates::metadata::parse_members;
#[test]
fn membership_is_explicit_and_malformed_membership_fails_closed() {
    let member = r#"{"id":"app-id","name":"app","manifest_path":"/ws/Cargo.toml","targets":[],"features":{},"dependencies":[]}"#;
    let json = format!(
        r#"{{"workspace_root":"/ws","workspace_members":["app-id"],"packages":[{member},{{"id":"external-id"}}]}}"#
    );
    let parsed = parse_members(&json).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].name, "app");
    for bad in [
        json.replace("[\"app-id\"]", "[\"absent-id\"]"),
        json.replace("[\"app-id\"]", "[\"app-id\",\"app-id\"]"),
    ] {
        assert!(parse_members(&bad).unwrap_err().contains("cargo metadata"));
    }
}
