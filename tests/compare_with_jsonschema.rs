use light_json_schema::{LightSchema, ValidationOptions};
use serde_json::json;

#[test]
fn test_comparison_with_jsonschema() {
    let schema_json = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 2, "maxLength": 10, "pattern": "^[a-z]+$" },
            "age": { "type": "integer", "minimum": 18, "maximum": 120 },
            "email": { "type": "string", "format": "email" },
            "tags": { "type": "array", "items": { "type": "string" }, "maxItems": 5 }
        },
        "required": ["name", "age", "email"]
    });

    let light_validator = LightSchema::parse(&schema_json).expect("Invalid light-json-schema");
    
    // In `jsonschema` crate, draft 7 format validation is disabled by default.
    // We can enable it via options, but it's simpler to just match its default behavior
    // by turning off format_assertions in light_validator too.
    let jsonschema_validator = jsonschema::validator_for(&schema_json).unwrap();
    
    let options = ValidationOptions {
        format_assertions: false,
        ..Default::default()
    };

    let test_cases = vec![
        (
            "Valid payload",
            json!({
                "name": "alice",
                "age": 30,
                "email": "alice@example.com",
                "tags": ["rust", "json"]
            })
        ),
        (
            "Missing required",
            json!({
                "name": "alice",
                "email": "alice@example.com"
            })
        ),
        (
            "Invalid type",
            json!({
                "name": "alice",
                "age": "30",
                "email": "alice@example.com"
            })
        ),
        (
            "Invalid length",
            json!({
                "name": "a",
                "age": 30,
                "email": "alice@example.com"
            })
        ),
        (
            "Invalid pattern",
            json!({
                "name": "Alice",
                "age": 30,
                "email": "alice@example.com"
            })
        ),
        (
            "Invalid format (should pass when format_assertions=false)",
            json!({
                "name": "alice",
                "age": 30,
                "email": "not-an-email"
            })
        ),
        (
            "Invalid array",
            json!({
                "name": "alice",
                "age": 30,
                "email": "alice@example.com",
                "tags": ["a", "b", "c", "d", "e", "f"]
            })
        ),
        (
            "Invalid array item type",
            json!({
                "name": "alice",
                "age": 30,
                "email": "alice@example.com",
                "tags": ["a", 2]
            })
        )
    ];

    for (name, payload) in test_cases {
        let is_valid_jsonschema = jsonschema_validator.is_valid(&payload);
        let is_valid_light = light_validator.validate(&payload, None, Some(options.clone())).is_valid;
        
        assert_eq!(
            is_valid_jsonschema, is_valid_light,
            "Mismatch on test case '{}'. jsonschema: {}, light-json-schema: {}",
            name, is_valid_jsonschema, is_valid_light
        );
    }
}
