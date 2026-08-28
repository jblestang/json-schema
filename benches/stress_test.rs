use criterion::{Criterion, Throughput, criterion_group, criterion_main};
use light_json_schema::{LightSchema, ValidationOptions};
use serde_json::json;

fn bench_stress_test(c: &mut Criterion) {
    let mut group = c.benchmark_group("light-json-schema-stress");

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

    let invalid_json = json!({
        "name": "InvalidNameIsTooLong",
        "age": "25",
        "email": "not-an-email",
        "tags": ["a", "b", "c", "d", "e", "f"],
        "extra_prop": 1
    });

    let payload_str = invalid_json.to_string();
    let bytes_len = payload_str.len();

    let schema = LightSchema::parse(&schema_json).unwrap();
    let options_fast = ValidationOptions {
        draft: light_json_schema::Draft::Draft7,
        stop_on_first_error: true,
        ..Default::default()
    };

    group.throughput(Throughput::Bytes(bytes_len as u64));
    group.bench_function("validate_huge_invalid", |b| {
        b.iter(|| {
            let result = schema.validate(
                std::hint::black_box(&invalid_json),
                None,
                Some(options_fast.clone()),
            );
            std::hint::black_box(result);
        })
    });

    // Now a valid case
    let mut valid_json = json!({});
    for _ in 0..100 {
        valid_json = json!({
            "a": valid_json
        });
    }
    let valid_bytes_len = valid_json.to_string().len();
    group.throughput(Throughput::Bytes(valid_bytes_len as u64));
    group.bench_function("validate_huge_valid", |b| {
        b.iter(|| {
            let result = schema.validate(
                std::hint::black_box(&valid_json),
                None,
                Some(options_fast.clone()),
            );
            std::hint::black_box(result);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_stress_test);
criterion_main!(benches);
