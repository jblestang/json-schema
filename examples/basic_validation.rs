use light_json_schema::LightSchema;
use serde_json::json;

fn main() {
    // 1. Define your JSON Schema
    let schema_json = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "minLength": 2 },
            "age": { "type": "integer", "minimum": 18 },
            "email": { "type": "string", "format": "email" }
        },
        "required": ["name", "email"]
    });

    // 2. Parse the schema (this processes all constraints into a fast, validated format)
    let schema = LightSchema::parse(&schema_json).unwrap();

    // 3. Define the payload you want to validate
    let payload = json!({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com"
    });

    // 4. Validate!
    let output = schema.validate(&payload, None, None);
    if output.is_valid {
        println!("Payload is valid!");
    } else {
        println!("Validation failed: {:?}", output.errors);
    }

    // Example of invalid payload
    let bad_payload = json!({
        "name": "A",          // Too short
        "age": 16,            // Too young
        "email": "not-email"  // Invalid format
    });

    println!("\nValidating bad payload:");
    let output2 = schema.validate(&bad_payload, None, None);
    if output2.is_valid {
        println!("Payload is valid!");
    } else {
        println!("Validation failed as expected!");
        for err in output2.errors {
            println!("- {}", err);
        }
    }
}
