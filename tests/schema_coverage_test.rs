use light_json_schema::{LightSchema, SchemaRegistry, ValidationError, ValidationOptions};
use serde_json::json;

#[test]
fn test_comprehensive_object_constraints() {
    let schema_json = json!({
        "type": "object",
        "properties": {
            "a": { "type": "string" },
            "b": { "type": "integer" }
        },
        "required": ["a"],
        "minProperties": 1,
        "maxProperties": 2,
        "propertyNames": { "pattern": "^[a-z]+$" },
        "dependentRequired": {
            "a": ["b"]
        },
        "dependentSchemas": {
            "b": { "properties": { "b": { "type": "integer" } } }
        },
        "additionalProperties": { "type": "boolean" }
    });

    let reg = SchemaRegistry::new();

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(
        schema
            .validate(&json!({"a": "hi", "b": 10}), Some(&reg), None)
            .is_valid
    );
    assert!(
        schema
            .validate(&json!({"a": "hi", "b": 10, "c": true}), Some(&reg), None)
            .is_valid
            == false
    ); // > 2 props
    assert!(
        schema
            .validate(&json!({"a": "hi"}), Some(&reg), None)
            .is_valid
            == false
    ); // dependentRequired on 'a' needs 'b'
    assert!(
        schema
            .validate(&json!({"A": "hi", "b": 10}), Some(&reg), None)
            .is_valid
            == false
    ); // propertyNames
    assert!(
        schema
            .validate(&json!({"a": "hi", "b": "bad"}), Some(&reg), None)
            .is_valid
            == false
    ); // dependentSchemas
    assert!(
        schema
            .validate(
                &json!({"a": "hi", "b": 10, "c": "not_bool"}),
                Some(&reg),
                None
            )
            .is_valid
            == false
    ); // additionalProperties schema
    assert!(schema.validate(&json!({}), Some(&reg), None).is_valid == false); // minProperties
}

#[test]
fn test_comprehensive_array_constraints() {
    let schema_json = json!({
        "type": "array",
        "items": { "type": "integer" },
        "minItems": 2,
        "maxItems": 4,
        "uniqueItems": true
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!([1, 2, 3]), None, None).is_valid);
    assert!(schema.validate(&json!([1, 1, 2]), None, None).is_valid == false); // uniqueItems
    assert!(schema.validate(&json!([1]), None, None).is_valid == false); // minItems
    assert!(
        schema
            .validate(&json!([1, 2, 3, 4, 5]), None, None)
            .is_valid
            == false
    ); // maxItems
}

#[test]
fn test_comprehensive_numeric_constraints() {
    let schema_json = json!({
        "type": "number",
        "minimum": 10.0,
        "maximum": 20.0,
        "multipleOf": 2.5
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(10.0), None, None).is_valid);
    assert!(schema.validate(&json!(12.5), None, None).is_valid);
    assert!(schema.validate(&json!(9.9), None, None).is_valid == false);
    assert!(schema.validate(&json!(20.1), None, None).is_valid == false);
    assert!(schema.validate(&json!(11.0), None, None).is_valid == false);
}

#[test]
fn test_exclusive_numeric_constraints() {
    let schema_json = json!({
        "type": "number",
        "exclusiveMinimum": 10.0,
        "exclusiveMaximum": 20.0
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(15.0), None, None).is_valid);
    assert!(schema.validate(&json!(10.0), None, None).is_valid == false);
    assert!(schema.validate(&json!(20.0), None, None).is_valid == false);
}

#[test]
fn test_comprehensive_string_constraints() {
    let schema_json = json!({
        "type": "string",
        "minLength": 3,
        "maxLength": 5,
        "pattern": "^a.*b$"
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!("a_b"), None, None).is_valid);
    assert!(schema.validate(&json!("ab"), None, None).is_valid == false);
    assert!(schema.validate(&json!("a_123_b"), None, None).is_valid == false);
    assert!(schema.validate(&json!("c_d"), None, None).is_valid == false);
}

#[test]
fn test_formats() {
    let formats = ["ipv4", "ipv6", "uri", "email", "date-time"];
    let valids = [
        "192.168.1.1",
        "2001:db8::1",
        "https://example.com/foo",
        "user@example.com",
        "2023-10-01T12:00:00Z",
    ];
    let invalids = [
        "192.168.1.300",
        "2001:xyz",
        "not_a_uri",
        "user@.com",
        "2023/10/01",
    ];

    for i in 0..formats.len() {
        let schema_json = json!({
            "type": "string",
            "format": formats[i]
        });

        let schema = LightSchema::parse(&schema_json).unwrap();

        let bad_schema = json!({
            "type": "object",
            "patternProperties": {
                "[": { "type": "string" }
            }
        });
        assert!(LightSchema::parse(&bad_schema).is_err());

        assert!(schema.validate(&json!(valids[i]), None, None).is_valid);
        assert!(
            schema
                .validate(
                    &json!(invalids[i]),
                    None,
                    Some(ValidationOptions {
                        format_assertions: true,
                        ..Default::default()
                    })
                )
                .is_valid
                == false
        );
    }
}

#[test]
fn test_logic() {
    let schema_json = json!({
        "anyOf": [{ "type": "string" }, { "type": "integer" }],
        "allOf": [{ "not": { "type": "boolean" } }],
        "oneOf": [{ "minimum": 10.0 }, { "maximum": 5.0 }]
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(15), None, None).is_valid);
    assert!(schema.validate(&json!(2), None, None).is_valid);
    assert!(schema.validate(&json!(7), None, None).is_valid == false); // neither oneOf
    assert!(schema.validate(&json!("test"), None, None).is_valid == false); // fails oneOf (not number so min/max doesn't apply? Wait, if type is not specified, minimum doesn't fail on strings! Ah. In draft 7, minimum ignores strings. So strings pass both branches of oneOf => 2 matches => fails oneOf)

    assert!(schema.validate(&json!(true), None, None).is_valid == false); // fails allOf
}

#[test]
fn test_conditionals() {
    let schema_json = json!({
        "if": { "type": "integer" },
        "then": { "minimum": 10.0 },
        "else": { "type": "string" }
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(15), None, None).is_valid);
    assert!(schema.validate(&json!(5), None, None).is_valid == false);
    assert!(schema.validate(&json!("hi"), None, None).is_valid);
    assert!(schema.validate(&json!(true), None, None).is_valid == false);
}

#[test]
fn test_references() {
    let mut registry = SchemaRegistry::new();
    registry
        .add("http://example.com/int", r#"{"type": "integer"}"#)
        .unwrap();

    let schema_json = json!({
        "$ref": "http://example.com/int"
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(10), Some(&registry), None).is_valid);
    assert!(
        schema
            .validate(&json!("hi"), Some(&registry), None)
            .is_valid
            == false
    );
    assert!(schema.validate(&json!(10), None, None).is_valid == false); // missing registry
}

#[test]
fn test_enum_const() {
    let schema_json = json!({
        "enum": ["a", 1],
        "const": 1
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(1), None, None).is_valid);
    assert!(schema.validate(&json!("a"), None, None).is_valid == false); // fails const
}

#[test]
fn test_booleans() {
    let schema = LightSchema::parse(&json!(true)).unwrap();
    assert!(schema.validate(&json!(1), None, None).is_valid);

    let schema = LightSchema::parse(&json!(false)).unwrap();
    assert!(schema.validate(&json!(1), None, None).is_valid == false);
}

#[test]
fn test_metadata_and_edge_cases() {
    let schema_json = json!({
        "title": "Test",
        "description": "Desc",
        "default": 42,
        "examples": [42]
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert_eq!(schema.title.unwrap(), "Test");
    assert_eq!(schema.description.unwrap(), "Desc");
    assert_eq!(schema.default.unwrap(), json!(42));
    assert_eq!(schema.examples.unwrap().len(), 1);

    // contains with maxContains
    let schema_json = json!({
        "type": "array",
        "contains": { "type": "integer" },
        "maxContains": 1
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!([1]), None, None).is_valid);
    assert!(schema.validate(&json!([1, 2]), None, None).is_valid == false); // > 1

    // exclusiveMaximum
    let schema_json = json!({
        "exclusiveMaximum": 10.0
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(9.9), None, None).is_valid);
    assert!(schema.validate(&json!(10.0), None, None).is_valid == false);

    // pattern missing match
    let schema_json = json!({
        "pattern": "^[a-z]+$"
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!("abc"), None, None).is_valid);
    assert!(schema.validate(&json!("123"), None, None).is_valid == false);

    // array type check
    let schema_json = json!({ "type": "array" });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!("not_array"), None, None).is_valid == false);

    // null type check
    let schema_json = json!({ "type": "null" });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    assert!(schema.validate(&json!(null), None, None).is_valid);
    assert!(schema.validate(&json!(1), None, None).is_valid == false);
}

#[test]
fn test_validation_error_display() {
    let schema_json = json!({
        "type": "object",
        "properties": {
            "users": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "age": { "type": "integer" }
                    }
                }
            }
        }
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    let data = json!({
        "users": [
            { "age": "not an integer" }
        ]
    });

    let result = schema.validate(&data, None, None);
    assert!(result.is_valid == false);

    let errs = result.errors;
    let display = format!("{}", errs[1]);
    assert_eq!(display, "$.users[0].age: Expected type Integer");
}

#[test]
fn test_stop_on_first_error() {
    let schema_json = json!({
        "type": "object",
        "properties": {
            "a": { "type": "integer" },
            "b": { "type": "integer" }
        }
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    let data = json!({ "a": "bad", "b": "bad" });

    let result_all = schema.validate(&data, None, None);
    assert_eq!(result_all.errors.len(), 2);

    let options = ValidationOptions {
        stop_on_first_error: true,
        max_depth: 16,
        format_assertions: false,
    };
    let result_early = schema.validate(&data, None, Some(options));
    assert_eq!(result_early.errors.len(), 1);
}

#[test]
fn test_unevaluated_items_failure() {
    let schema_json = json!({
        "type": "array",
        "items": [
            { "type": "string" }
        ],
        "unevaluatedItems": { "type": "integer" }
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    // First is evaluated by items, second is evaluated by unevaluatedItems (which fails because it's a string)
    let data = json!(["hello", "world"]);
    assert!(schema.validate(&data, None, None).is_valid == false);

    let data2 = json!(["hello", 123]);
    assert!(schema.validate(&data2, None, None).is_valid);
}

#[test]
fn test_unevaluated_properties_failure() {
    let schema_json = json!({
        "type": "object",
        "properties": {
            "a": { "type": "string" }
        },
        "unevaluatedProperties": { "type": "integer" }
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    // b fails because unevaluated properties must be integers
    let data = json!({ "a": "hello", "b": "world" });
    assert!(schema.validate(&data, None, None).is_valid == false);

    let data2 = json!({ "a": "hello", "b": 123 });
    assert!(schema.validate(&data2, None, None).is_valid);
}

#[test]
fn test_complex_coverage() {
    let schema_json = json!({
        "type": "object",
        "patternProperties": {
            "^[0-9]+$": { "type": "integer" },

        },
        "additionalProperties": { "type": "boolean" },
        "properties": {
            "arr": {
                "type": "array",
                "prefixItems": [
                    { "type": "string" }
                ],
                "contains": { "type": "integer" }
            }
        }
    });

    // The invalid regex "[": { "type": "string" } is skipped during validation

    let schema = LightSchema::parse(&schema_json).unwrap();

    let bad_schema = json!({
        "type": "object",
        "patternProperties": {
            "[": { "type": "string" }
        }
    });
    assert!(LightSchema::parse(&bad_schema).is_err());

    // Test patternProperties failure (string instead of integer)
    let data1 = json!({ "123": "not_an_integer" });
    assert!(schema.validate(&data1, None, None).is_valid == false);

    // Test additionalProperties failure (string instead of boolean)
    let data2 = json!({ "abc": "not_a_boolean" });
    assert!(schema.validate(&data2, None, None).is_valid == false);

    // Test prefixItems failure
    let data3 = json!({ "arr": [ 123 ] }); // 123 is integer, but prefix item 0 is string
    assert!(schema.validate(&data3, None, None).is_valid == false);

    // Test contains failure
    let data4 = json!({ "arr": [ "hi", "there" ] }); // no integers inside
    assert!(schema.validate(&data4, None, None).is_valid == false);
}

#[test]
fn test_max_depth_exceeded() {
    let schema_json = json!({
        "$ref": "self",
        "type": "object",
        "properties": {
            "a": { "$ref": "self" }
        }
    });
    let mut registry = SchemaRegistry::new();
    registry.add("self", &schema_json.to_string()).unwrap();
    let schema = LightSchema::parse(&schema_json).unwrap();

    // Default max depth is 16. A deeply nested json should hit it.
    let mut val = json!({});
    for _ in 0..20 {
        val = json!({ "a": val });
    }

    let result = schema.validate(&val, Some(&registry), None);
    assert!(!result.is_valid, "{:?}", result.errors);
    assert!(
        result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::MaxDepthExceeded))
    );
}

#[test]
fn test_unresolved_dynamic_ref() {
    let schema_json = json!({
        "$dynamicRef": "missing_ref"
    });
    let schema = LightSchema::parse(&schema_json).unwrap();
    let result = schema.validate(&json!(1), None, None);
    assert!(!result.is_valid, "{:?}", result.errors);
}

#[test]
fn test_type_arrays() {
    let schema_json = json!({
        "type": ["string", "null"]
    });
    let schema = LightSchema::parse(&schema_json).unwrap();
    assert!(schema.validate(&json!("hi"), None, None).is_valid);
    assert!(schema.validate(&json!(null), None, None).is_valid);
    assert!(!schema.validate(&json!(1), None, None).is_valid);
}

#[test]
fn test_format_warnings() {
    let schema_json = json!({
        "type": "string",
        "format": "ipv4"
    });
    let schema = LightSchema::parse(&schema_json).unwrap();

    let result = schema.validate(&json!("not-an-ip"), None, None);
    assert!(result.is_valid, "{:?}", result.errors);
    assert_eq!(result.warnings.len(), 1);

    let opts = ValidationOptions {
        format_assertions: true,
        ..Default::default()
    };
    let result_err = schema.validate(&json!("not-an-ip"), None, Some(opts));
    assert!(!result_err.is_valid);
    assert_eq!(result_err.errors.len(), 1);
}
