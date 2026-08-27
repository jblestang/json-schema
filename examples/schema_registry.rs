use light_json_schema::{LightSchema, SchemaRegistry};
use serde_json::json;

fn main() {
    // 1. Create a SchemaRegistry.
    // Since light-json-schema is completely no_std and performs zero I/O,
    // you must explicitly provide remote schemas via a registry.
    let mut registry = SchemaRegistry::new();

    // 2. Add an external schema to the registry
    registry
        .add(
            "https://example.com/address.json",
            r#"{
        "type": "object",
        "properties": {
            "street": { "type": "string" },
            "city": { "type": "string" }
        },
        "required": ["street", "city"]
    }"#,
        )
        .expect("Failed to parse address schema");

    // 3. Define your main schema that references the external schema
    let schema_json = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "billing_address": { "$ref": "https://example.com/address.json" },
            "shipping_address": { "$ref": "https://example.com/address.json" }
        },
        "required": ["name", "billing_address"]
    });

    let schema = LightSchema::parse(&schema_json).unwrap();

    // 4. Validate! Notice we pass `Some(&registry)` so the validator can resolve the $ref's
    let payload = json!({
        "name": "Bob",
        "billing_address": {
            "street": "123 Rust Lane",
            "city": " Ferris City"
        }
    });

    println!("Validating good payload with external refs:");
    let output = schema.validate(&payload, Some(&registry), None);
    if output.is_valid {
        println!("Payload is valid!");
    } else {
        println!("Validation failed: {:?}", output.errors);
    }

    // Example of invalid payload
    let bad_payload = json!({
        "name": "Bob",
        "billing_address": {
            "street": "123 Rust Lane" // missing 'city'
        }
    });

    println!("\nValidating bad payload (missing field in $ref):");
    let output2 = schema.validate(&bad_payload, Some(&registry), None);
    if output2.is_valid {
        println!("Payload is valid!");
    } else {
        println!("Validation failed as expected!");
        for err in output2.errors {
            println!("- {}", err);
        }
    }
}
