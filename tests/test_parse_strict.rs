use light_json_schema::LightSchema;
use serde_json::Value;

#[test]
fn test_parse_strict_valid() {
    let schema_json = r#"{"type": "string"}"#;
    let val: Value = serde_json::from_str(schema_json).unwrap();
    let schema = LightSchema::parse_strict(&val);
    if let Err(e) = &schema {
        println!("Error: {:?}", e);
    }
    assert!(schema.is_ok());
}

#[test]
fn test_parse_strict_invalid() {
    let schema_json = r#"{"type": "invalid_type"}"#;
    let val: Value = serde_json::from_str(schema_json).unwrap();
    let schema = LightSchema::parse_strict(&val);
    assert!(schema.is_err());
    match schema.unwrap_err() {
        light_json_schema::SchemaParseError::InvalidSchema(errs) => {
            assert!(!errs.is_empty());
        }
        _ => panic!("Expected InvalidSchema error"),
    }
}
