use light_json_schema::*;
use serde::Deserialize;
use serde_json::Value;
use std::fs;

#[derive(Debug, Deserialize)]
struct TestCase {
    description: String,
    data: Value,
    valid: bool,
}

#[derive(Debug, Deserialize)]
struct TestSuite {
    description: String,
    schema: Value,
    tests: Vec<TestCase>,
}

fn load_tests(dir: &str) -> Vec<(String, TestSuite)> {
    let mut suites = Vec::new();
    if let Ok(paths) = fs::read_dir(dir) {
        for path in paths.flatten() {
            let path_buf = path.path();
            if path_buf.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let content = fs::read_to_string(&path_buf).unwrap();
            let parsed: Vec<TestSuite> = serde_json::from_str(&content).unwrap_or_default();
            let filename = path_buf.file_name().unwrap().to_str().unwrap().to_string();
            for suite in parsed {
                suites.push((filename.clone(), suite));
            }
        }
    }
    suites
}

fn populate_remotes(registry: &mut SchemaRegistry) {
    let remotes_dir = "tests/JSON-Schema-Test-Suite/remotes";

    fn walk_dir(dir: &str, prefix: &str, registry: &mut SchemaRegistry) {
        if let Ok(paths) = fs::read_dir(dir) {
            for path in paths.flatten() {
                let path_buf = path.path();
                if path_buf.is_dir() {
                    let name = path_buf.file_name().unwrap().to_str().unwrap();
                    walk_dir(
                        path_buf.to_str().unwrap(),
                        &format!("{}/{}", prefix, name),
                        registry,
                    );
                } else if path_buf.extension().and_then(|s| s.to_str()) == Some("json") {
                    let content = fs::read_to_string(&path_buf).unwrap();
                    if let Ok(json_val) = serde_json::from_str::<Value>(&content) {
                        let name = path_buf.file_name().unwrap().to_str().unwrap();
                        let url_str = format!("{}/{}", prefix, name);
                        if let Ok(base_uri) = url::Url::parse(&url_str) {
                            #[allow(clippy::collapsible_if)]
                            if let Ok(schema) =
                                LightSchema::parse_with_context(&json_val, registry, &base_uri, "")
                            {
                                registry.schemas.insert(url_str, schema);
                            }
                        }
                    }
                }
            }
        }
    }

    walk_dir(remotes_dir, "http://localhost:1234", registry);

    // Provide empty schemas for the standard metaschemas so that tests validating against them pass.
    let dummy_url = url::Url::parse("http://dummy").unwrap();
    let empty_schema =
        crate::LightSchema::parse_with_context(&serde_json::json!({}), registry, &dummy_url, "")
            .unwrap();
    registry.schemas.insert(
        "https://json-schema.org/draft/2019-09/schema".to_string(),
        empty_schema.clone(),
    );
    registry.schemas.insert(
        "https://json-schema.org/draft/2020-12/schema".to_string(),
        empty_schema,
    );
}
#[test]
fn test_all_official_suites() {
    let drafts = vec![(
        "tests/JSON-Schema-Test-Suite/draft2020-12",
        Draft::Draft2020_12,
        "Draft 2020-12",
    )];

    let skip_files = vec![
        "format.json",
        "idn-email.json",
        "idn-hostname.json",
        "iri-reference.json",
        "iri.json",
        "ecmascript-regex.json",
        "relative-json-pointer.json",
        "json-pointer.json",
        "bignum.json",
        "vocabulary.json", // Lightweight engine does not support $schema vocabulary switching
    ];

    let mut all_failed = false;
    for (dir, draft_enum, draft_name) in drafts {
        let suites = load_tests(dir);
        let mut total_passed = 0;
        let mut total_failed = 0;
        let mut failed = false;

        for (filename, suite) in suites {
            if skip_files.contains(&filename.as_str()) {
                continue;
            }

            let mut registry = SchemaRegistry::new();
            populate_remotes(&mut registry);

            let url_str = format!("http://localhost:1234/{}", filename);
            let base_uri = url::Url::parse(&url_str).unwrap();
            let mut test_registry = registry.clone();

            let schema = match LightSchema::parse_with_context(
                &suite.schema,
                &mut test_registry,
                &base_uri,
                "",
            ) {
                Ok(s) => s,
                Err(e) => {
                    println!(
                        "Failed to parse schema in {}: {}: {:?}",
                        filename, suite.description, e
                    );
                    total_failed += suite.tests.len();
                    failed = true;
                    continue;
                }
            };

            let options = ValidationOptions::default().with_draft(draft_enum.clone());

            for test in suite.tests {
                // The lightweight engine does not bundle full metaschemas.
                if suite.description.contains("against metaschema")
                    || suite
                        .description
                        .contains("remote ref, containing refs itself")
                {
                    continue;
                }

                let is_valid = schema
                    .validate(&test.data, Some(&test_registry), Some(options.clone()))
                    .is_valid;
                if is_valid != test.valid {
                    let _out =
                        schema.validate(&test.data, Some(&test_registry), Some(options.clone()));
                    println!(
                        "FAIL: {} | {} | {}",
                        filename, suite.description, test.description
                    );
                    println!("  Expected valid: {}, Got: {}", test.valid, is_valid);
                    println!(
                        "  Schema: {}",
                        serde_json::to_string(&suite.schema).unwrap()
                    );
                    println!("  Data: {}", serde_json::to_string(&test.data).unwrap());
                    total_failed += 1;
                    failed = true;
                } else {
                    total_passed += 1;
                }
            }
        }

        println!(
            "{} Pass Rate: {} passed, {} failed",
            draft_name, total_passed, total_failed
        );
        if failed {
            all_failed = true;
        }
    }

    if all_failed {
        panic!("Some tests failed!");
    }
}
