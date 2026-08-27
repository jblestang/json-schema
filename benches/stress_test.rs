use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use light_json_schema::{LightSchema, ValidationOptions};
use serde_json::json;

fn bench_stress_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("Validation Stress Test");

    // A small JSON schema with constraints
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

    let schema = LightSchema::parse(&schema_json).unwrap();

    let invalid_json = json!({
        "name": "InvalidNameIsTooLong",
        "age": "25",
        "email": "not-an-email",
        "tags": ["a", "b", "c", "d", "e", "f"],
        "extra_prop": 1
    });

    let payload_str = invalid_json.to_string();
    let bytes_len = payload_str.as_bytes().len();
    group.throughput(Throughput::Bytes(bytes_len as u64));

    // Measure with fast fail
    let options_fast = ValidationOptions {
        format_assertions: true,
        max_depth: 32,
        stop_on_first_error: true,
    };

    group.bench_function("validate_small_json_errors_fast_fail", |b| {
        b.iter(|| {
            let result =
                schema.validate(black_box(&invalid_json), None, Some(options_fast.clone()));
            black_box(result);
        });
    });

    // Valid JSON benchmark to see maximum potential throughput
    let valid_json = json!({
        "name": "valid",
        "age": 25,
        "email": "test@example.com",
        "tags": ["rust", "json"]
    });
    let valid_bytes_len = valid_json.to_string().as_bytes().len();
    group.throughput(Throughput::Bytes(valid_bytes_len as u64));
    group.bench_function("validate_small_json_valid", |b| {
        b.iter(|| {
            let result = schema.validate(black_box(&valid_json), None, Some(options_fast.clone()));
            black_box(result);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_stress_test);
criterion_main!(benches);
