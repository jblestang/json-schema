# light-json-schema

A lightning-fast, zero-allocation (mostly), `no_std`-compatible JSON schema validation engine for Rust.

Built with extreme performance and determinism in mind, `light-json-schema` is designed for environments where standard JSON validation libraries are too heavy, allocate too much, or don't support `no_std`.

## Features

- **`no_std` Compatible**: Can be used in embedded devices or WebAssembly without the Rust standard library (requires `alloc`).
- **High Performance**: Pre-compiles the schema into an optimized internal representation. Throughput can exceed 600 MB/s for fast-fail validations.
- **Zero Panic Guarantee**: Carefully constructed to never panic on malicious or deeply nested JSON (protects against stack overflows with configurable max depth).
- **Format Validation**: Includes built-in support for `ipv4`, `ipv6`, `uri`, `email`, and `date-time` formats. Unknown formats are rejected at schema parse time, just like invalid regexes.
- **Robust References**: Full support for `$ref` and `$dynamicRef` across local registries.
- **Flexible Options**: Distinguish between fatal validation errors and non-fatal warnings (e.g., format assertions can be configured to just warn).
- **Early Exit**: `stop_on_first_error` option to immediately bail on the first validation failure, drastically improving throughput on bad payloads.

## Usage

```rust
use light_json_schema::{LightSchema, ValidationOptions};
use serde_json::json;

fn main() {
    let schema_json = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string", "maxLength": 10 },
            "age": { "type": "integer", "minimum": 18 },
            "email": { "type": "string", "format": "email" }
        },
        "required": ["name", "age"]
    });

    // Parse the schema once
    let schema = LightSchema::parse(&schema_json).expect("Invalid schema");

    let valid_data = json!({
        "name": "Alice",
        "age": 30,
        "email": "alice@example.com"
    });

    // Validate payloads efficiently
    let options = ValidationOptions {
        format_assertions: true,
        stop_on_first_error: true,
        max_depth: 32,
    };
    
    let result = schema.validate(&valid_data, None, Some(options));
    assert!(result.is_valid());
}
```

## Performance

In our criterion stress tests on small objects:
- Fast-fail validation on invalid payloads reaches ~650 MB/s.
- Validation on valid payloads reaches ~90 MB/s.
- Exhaustive validation (collecting all errors) reaches ~93 MB/s.

## License
Dual-licensed under MIT or Apache 2.0.
