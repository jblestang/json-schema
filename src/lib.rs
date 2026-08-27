#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde_json::Value;

use core::net::{Ipv4Addr, Ipv6Addr};
use core::str::FromStr;
use url::Url;

/// Options to configure the validation behavior.
#[derive(Debug, Clone)]
pub struct ValidationOptions {
    pub stop_on_first_error: bool,
    pub max_depth: usize,
    pub format_assertions: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            stop_on_first_error: false,
            max_depth: 16,
            format_assertions: false,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ValidationOutput {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationError>,
}

/// Tracks the state of validation across complex schema compositions.
///
/// In JSON Schema (Draft 2020-12 and newer), `unevaluatedProperties` and
/// `unevaluatedItems` require knowing exactly which keys or array indices
/// were successfully evaluated by adjacent schemas (e.g., in `allOf`, `$ref`).
/// This struct acts as a mutable trace of that state during validation.
#[derive(Debug, Clone, Default)]
pub struct EvaluationState {
    pub evaluated_properties: BTreeSet<String>,
    pub evaluated_items: BTreeSet<usize>,
}

impl EvaluationState {
    /// Merges the evaluation state from another validation branch into this one.
    /// This is heavily used when combining results from `allOf`, `anyOf`, or `$ref`.
    pub fn merge(&mut self, other: &EvaluationState) {
        self.evaluated_properties
            .extend(other.evaluated_properties.iter().cloned());
        self.evaluated_items
            .extend(other.evaluated_items.iter().copied());
    }
}

/// A zero-I/O registry for resolving external JSON schema references (`$ref`).
///
/// Because `light-json-schema` is designed for `#![no_std]` environments, it
/// cannot perform network HTTP requests or read from the filesystem to resolve
/// remote schemas. Instead, you must pre-load remote schemas into this registry.
#[derive(Default)]
pub struct SchemaRegistry {
    pub schemas: BTreeMap<String, LightSchema>,
}

impl SchemaRegistry {
    /// Creates a new, empty SchemaRegistry.
    pub fn new() -> Self {
        Self {
            schemas: BTreeMap::new(),
        }
    }

    /// Parses a JSON schema from a string and registers it under the provided ID.
    ///
    /// # Arguments
    /// * `id` - The URI or string identifier that `$ref` keywords will use.
    /// * `schema_json` - The raw JSON string of the schema.
    pub fn add(&mut self, id: &str, schema_json: &str) -> Result<(), String> {
        let val: Value =
            serde_json::from_str(schema_json).map_err(|e| format!("Invalid JSON: {}", e))?;
        let schema = LightSchema::parse(&val).map_err(|e| format!("Schema parse error: {}", e))?;
        self.schemas.insert(id.to_string(), schema);
        Ok(())
    }
}

/// Represents the core JSON data types defined by the JSON Schema specification.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaType {
    Object,
    String,
    Integer,
    Number,
    Boolean,
    Array,
    Null,
    Any,
}

#[derive(Debug, Clone)]
pub enum SchemaFormat {
    Ipv4,
    Ipv6,
    Uri,
    Email(regex::Regex),
    DateTime,
}

#[derive(Debug, Clone)]
pub enum SchemaParseError {
    InvalidRegex(String),
    UnknownFormat(String),
}

impl core::fmt::Display for SchemaParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SchemaParseError::InvalidRegex(e) => write!(f, "Invalid regular expression: {}", e),
            SchemaParseError::UnknownFormat(f_name) => {
                write!(f, "Unknown format string: {}", f_name)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ValidationError {
    UnresolvedReference(String),
    MaxDepthExceeded,
    NotInEnum,
    ConstMismatch,
    TypeMismatch(SchemaType),

    // Object
    MinProperties(usize),
    MaxProperties(usize),
    MissingRequired(String),
    MissingDependentRequired {
        req: String,
        dep: String,
    },
    DependentSchemaFailed(String),
    InvalidPropertyName(String),
    AdditionalPropertyNotAllowed(String),
    UnevaluatedPropertyFailed(String),

    // Array
    MinItems(usize),
    MaxItems(usize),
    NotUnique,
    PrefixItemFailed(usize),
    ItemFailed(usize),
    ContainsNoMatch,
    MinContains(usize),
    MaxContains(usize),
    UnevaluatedItemFailed(usize),

    // String
    MinLength(usize),
    MaxLength(usize),
    PatternMismatch,
    InvalidFormat(String),

    // Numeric
    Minimum(f64),
    Maximum(f64),
    ExclusiveMinimum(f64),
    ExclusiveMaximum(f64),
    MultipleOf(f64),

    // Logic
    AnyOfFailed,
    AllOfFailed,
    OneOfMatches(usize),
    NotFailed,
    ThenFailed,
    ElseFailed,

    // Generic sub-error wrapper
    SubSchemaFailed,

    // Path Context
    InProperty {
        key: String,
        error: Box<ValidationError>,
    },
    InIndex {
        index: usize,
        error: Box<ValidationError>,
    },
}

impl core::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Collect the path
        let mut path = alloc::vec::Vec::new();
        let mut current = self;

        while let ValidationError::InProperty { error, .. }
        | ValidationError::InIndex { error, .. } = current
        {
            if let ValidationError::InProperty { key, .. } = current {
                path.push(format!(".{}", key));
            } else if let ValidationError::InIndex { index, .. } = current {
                path.push(format!("[{}]", index));
            }
            current = error;
        }

        if !path.is_empty() {
            write!(f, "$")?;
            for p in path {
                write!(f, "{}", p)?;
            }
            write!(f, ": ")?;
        }

        // Print the actual error
        match current {
            ValidationError::TypeMismatch(t) => write!(f, "Expected type {:?}", t),
            ValidationError::MissingRequired(k) => write!(f, "Missing required field '{}'", k),
            ValidationError::MinLength(l) => write!(f, "String shorter than minLength {}", l),
            ValidationError::MaxLength(l) => write!(f, "String longer than maxLength {}", l),
            ValidationError::Minimum(m) => write!(f, "Value less than minimum {}", m),
            ValidationError::Maximum(m) => write!(f, "Value greater than maximum {}", m),
            ValidationError::InvalidFormat(fmt) => write!(f, "Invalid {} format", fmt),
            ValidationError::AdditionalPropertyNotAllowed(k) => {
                write!(f, "Additional property '{}' not allowed", k)
            }
            ValidationError::UnevaluatedPropertyFailed(k) => {
                write!(f, "Unevaluated property '{}' failed validation", k)
            }
            ValidationError::ItemFailed(i) => write!(f, "Array item {} invalid", i),
            ValidationError::NotUnique => write!(f, "Array items are not unique"),
            ValidationError::PatternMismatch => write!(f, "String does not match pattern"),
            ValidationError::NotInEnum => write!(f, "Value not in enum"),
            ValidationError::ConstMismatch => write!(f, "Value does not match const"),
            ValidationError::UnresolvedReference(r) => write!(f, "Unresolved reference: {}", r),
            ValidationError::MaxDepthExceeded => write!(f, "Maximum recursion depth exceeded"),
            _ => write!(f, "{:?}", current),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ObjectConstraints {
    pub properties: BTreeMap<String, LightSchema>,
    pub required: Vec<String>,
    pub additional_properties_allowed: Option<bool>,
    pub additional_properties_schema: Option<Box<LightSchema>>,
    pub unevaluated_properties: Option<Box<LightSchema>>,
    pub min_properties: Option<usize>,
    pub max_properties: Option<usize>,
    pub dependent_required: BTreeMap<String, Vec<String>>,
    pub dependent_schemas: BTreeMap<String, Box<LightSchema>>,
    pub pattern_properties: Vec<(regex::Regex, LightSchema)>,
    pub property_names: Option<Box<LightSchema>>,
}

#[derive(Debug, Clone)]
pub struct ArrayConstraints {
    pub items: Option<Box<LightSchema>>,
    pub prefix_items: Option<Vec<LightSchema>>,
    pub unevaluated_items: Option<Box<LightSchema>>,
    pub min_items: Option<usize>,
    pub max_items: Option<usize>,
    pub unique_items: Option<bool>,
    pub contains: Option<Box<LightSchema>>,
    pub min_contains: Option<usize>,
    pub max_contains: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct NumericConstraints {
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub exclusive_minimum: Option<f64>,
    pub exclusive_maximum: Option<f64>,
    pub multiple_of: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct StringConstraints {
    pub min_length: Option<usize>,
    pub max_length: Option<usize>,
    pub pattern: Option<regex::Regex>,
}

#[derive(Debug, Clone)]
pub struct LogicConstraints {
    pub any_of: Vec<LightSchema>,
    pub all_of: Vec<LightSchema>,
    pub one_of: Vec<LightSchema>,
    pub not: Option<Box<LightSchema>>,
    pub conditional_if: Option<Box<LightSchema>>,
    pub conditional_then: Option<Box<LightSchema>>,
    pub conditional_else: Option<Box<LightSchema>>,
}

/// The core parsed representation of a JSON Schema.
///
/// `LightSchema` pre-compiles and recursively extracts all constraints, logic gates,
/// and metadata from a raw JSON Schema definition. By parsing the schema once during
/// application startup, the validation phase is extremely fast and allocates minimally.
#[derive(Debug, Clone)]
pub struct LightSchema {
    pub types: Vec<SchemaType>,
    /// Points to an external or internal schema identifier for `$ref` resolution.
    pub reference: Option<String>,
    pub dynamic_reference: Option<String>,
    /// Specifies semantic validation (e.g. `ipv4`, `email`, `date-time`).
    pub format: Option<SchemaFormat>,

    // Metadata
    pub title: Option<String>,
    pub description: Option<String>,
    pub default: Option<Value>,
    pub examples: Option<Vec<Value>>,
    pub enum_values: Option<Vec<Value>>,
    pub const_value: Option<Value>,

    pub obj: Option<Box<ObjectConstraints>>,
    pub arr: Option<Box<ArrayConstraints>>,
    pub num: Option<Box<NumericConstraints>>,
    pub str: Option<Box<StringConstraints>>,
    pub log: Option<Box<LogicConstraints>>,
}

impl LightSchema {
    /// Parses a raw `serde_json::Value` into a strongly-typed `LightSchema`.
    /// This method recursively parses subschemas.
    pub fn parse(val: &Value) -> Result<Self, SchemaParseError> {
        // 1. Check for boolean schema (true = any, false = not any)
        if let Some(b) = val.as_bool() {
            let mut schema = Self::empty();
            if !b {
                let log = LogicConstraints {
                    any_of: Vec::new(),
                    all_of: Vec::new(),
                    one_of: Vec::new(),
                    not: Some(Box::new(Self::empty())),
                    conditional_if: None,
                    conditional_then: None,
                    conditional_else: None,
                };
                schema.log = Some(Box::new(log));
            }
            return Ok(schema);
        }

        // 2. Parse core types

        let mut types = Vec::new();
        if let Some(t) = val.get("type") {
            if let Some(s) = t.as_str() {
                types.push(match s {
                    "object" => SchemaType::Object,
                    "string" => SchemaType::String,
                    "integer" => SchemaType::Integer,
                    "number" => SchemaType::Number,
                    "boolean" => SchemaType::Boolean,
                    "array" => SchemaType::Array,
                    "null" => SchemaType::Null,
                    _ => SchemaType::Any,
                });
            } else if let Some(arr) = t.as_array() {
                for item in arr {
                    if let Some(s) = item.as_str() {
                        types.push(match s {
                            "object" => SchemaType::Object,
                            "string" => SchemaType::String,
                            "integer" => SchemaType::Integer,
                            "number" => SchemaType::Number,
                            "boolean" => SchemaType::Boolean,
                            "array" => SchemaType::Array,
                            "null" => SchemaType::Null,
                            _ => SchemaType::Any,
                        });
                    }
                }
            }
        } else {
            types.push(SchemaType::Any);
        }

        // 3. Metadata extraction
        let reference = val
            .get("$ref")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let dynamic_reference = val
            .get("$dynamicRef")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let format = match val.get("format").and_then(|v| v.as_str()) {
            Some("ipv4") => Some(SchemaFormat::Ipv4),
            Some("ipv6") => Some(SchemaFormat::Ipv6),
            Some("uri") => Some(SchemaFormat::Uri),
            Some("email") => Some(SchemaFormat::Email(
                regex::Regex::new(r"^[a-zA-Z0-9_.+-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9-.]+$").unwrap(),
            )),
            Some("date-time") => Some(SchemaFormat::DateTime),
            Some(s) => return Err(SchemaParseError::UnknownFormat(s.to_string())),
            None => None,
        };

        let title = val
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let description = val
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let default = val.get("default").cloned();
        let examples = val.get("examples").and_then(|v| v.as_array()).cloned();
        let enum_values = val.get("enum").and_then(|v| v.as_array()).cloned();
        let const_value = val.get("const").cloned();

        // 4. Object parsing
        let mut obj_constraints = None;
        let mut has_obj = false;

        let mut properties = BTreeMap::new();
        if let Some(props) = val.get("properties").and_then(|p| p.as_object()) {
            has_obj = true;
            for (k, v) in props {
                properties.insert(k.clone(), LightSchema::parse(v)?);
            }
        }

        let mut required = Vec::new();
        if let Some(req) = val.get("required").and_then(|r| r.as_array()) {
            has_obj = true;
            for item in req {
                if let Some(s) = item.as_str() {
                    required.push(s.to_string());
                }
            }
        }

        let mut dependent_required = BTreeMap::new();
        if let Some(deps) = val.get("dependentRequired").and_then(|p| p.as_object()) {
            has_obj = true;
            for (k, v) in deps {
                if let Some(arr) = v.as_array() {
                    let mut req_list = Vec::new();
                    for item in arr {
                        if let Some(s) = item.as_str() {
                            req_list.push(s.to_string());
                        }
                    }
                    dependent_required.insert(k.clone(), req_list);
                }
            }
        }

        let mut dependent_schemas = BTreeMap::new();
        if let Some(deps) = val.get("dependentSchemas").and_then(|p| p.as_object()) {
            has_obj = true;
            for (k, v) in deps {
                dependent_schemas.insert(k.clone(), Box::new(LightSchema::parse(v)?));
            }
        }

        let mut pattern_properties = Vec::new();
        if let Some(props) = val.get("patternProperties").and_then(|p| p.as_object()) {
            has_obj = true;
            for (k, v) in props {
                let reg = regex::Regex::new(k)
                    .map_err(|e| SchemaParseError::InvalidRegex(e.to_string()))?;
                pattern_properties.push((reg, LightSchema::parse(v)?));
            }
        }

        let property_names = match val.get("propertyNames") {
            Some(v) => {
                has_obj = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };
        let unevaluated_properties = match val.get("unevaluatedProperties") {
            Some(v) => {
                has_obj = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };

        // Handle additionalProperties (boolean or schema)
        let mut additional_properties_allowed = None;
        let mut additional_properties_schema = None;
        if let Some(ap) = val.get("additionalProperties") {
            has_obj = true;
            if let Some(b) = ap.as_bool() {
                additional_properties_allowed = Some(b);
            } else if ap.is_object() {
                additional_properties_schema = Some(Box::new(LightSchema::parse(ap)?));
            }
        }

        let min_properties = val
            .get("minProperties")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let max_properties = val
            .get("maxProperties")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        if min_properties.is_some() || max_properties.is_some() {
            has_obj = true;
        }

        if has_obj {
            obj_constraints = Some(Box::new(ObjectConstraints {
                properties,
                required,
                additional_properties_allowed,
                additional_properties_schema,
                unevaluated_properties,
                min_properties,
                max_properties,
                dependent_required,
                dependent_schemas,
                pattern_properties,
                property_names,
            }));
        }

        let mut arr_constraints = None;
        let mut has_arr = false;

        let mut items = None;
        let mut prefix_items = None;
        if let Some(it) = val.get("items") {
            has_arr = true;
            if let Some(arr_it) = it.as_array() {
                // Draft 07 arrays
                let mut prefix = Vec::new();
                for item in arr_it {
                    prefix.push(LightSchema::parse(item)?);
                }
                prefix_items = Some(prefix);
            } else {
                items = Some(Box::new(LightSchema::parse(it)?));
            }
        }
        if let Some(pre) = val.get("prefixItems").and_then(|a| a.as_array()) {
            has_arr = true;
            let mut prefix = Vec::new();
            for item in pre {
                prefix.push(LightSchema::parse(item)?);
            }
            prefix_items = Some(prefix);
        }

        let min_items = val
            .get("minItems")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let max_items = val
            .get("maxItems")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let unique_items = val.get("uniqueItems").and_then(|v| v.as_bool());
        if min_items.is_some() || max_items.is_some() || unique_items.is_some() {
            has_arr = true;
        }

        let unevaluated_items = match val.get("unevaluatedItems") {
            Some(v) => {
                has_arr = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };

        let contains = match val.get("contains") {
            Some(v) => {
                has_arr = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };
        let min_contains = val
            .get("minContains")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let max_contains = val
            .get("maxContains")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        if min_contains.is_some() || max_contains.is_some() {
            has_arr = true;
        }

        if has_arr {
            arr_constraints = Some(Box::new(ArrayConstraints {
                items,
                prefix_items,
                unevaluated_items,
                min_items,
                max_items,
                unique_items,
                contains,
                min_contains,
                max_contains,
            }));
        }

        let minimum = val.get("minimum").and_then(|v| v.as_f64());
        let maximum = val.get("maximum").and_then(|v| v.as_f64());
        let exclusive_minimum = val.get("exclusiveMinimum").and_then(|v| v.as_f64());
        let exclusive_maximum = val.get("exclusiveMaximum").and_then(|v| v.as_f64());
        let multiple_of = val.get("multipleOf").and_then(|v| v.as_f64());
        let num_constraints = if minimum.is_some()
            || maximum.is_some()
            || exclusive_minimum.is_some()
            || exclusive_maximum.is_some()
            || multiple_of.is_some()
        {
            Some(Box::new(NumericConstraints {
                minimum,
                maximum,
                exclusive_minimum,
                exclusive_maximum,
                multiple_of,
            }))
        } else {
            None
        };

        let min_length = val
            .get("minLength")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let max_length = val
            .get("maxLength")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);
        let mut pattern = None;
        if let Some(p) = val.get("pattern").and_then(|v| v.as_str()) {
            pattern = Some(
                regex::Regex::new(p).map_err(|e| SchemaParseError::InvalidRegex(e.to_string()))?,
            );
        }
        let str_constraints = if min_length.is_some() || max_length.is_some() || pattern.is_some() {
            Some(Box::new(StringConstraints {
                min_length,
                max_length,
                pattern,
            }))
        } else {
            None
        };

        let mut has_log = false;
        let mut any_of = Vec::new();
        if let Some(any) = val.get("anyOf").and_then(|a| a.as_array()) {
            has_log = true;
            for item in any {
                any_of.push(LightSchema::parse(item)?);
            }
        }
        let mut all_of = Vec::new();
        if let Some(all) = val.get("allOf").and_then(|a| a.as_array()) {
            has_log = true;
            for item in all {
                all_of.push(LightSchema::parse(item)?);
            }
        }
        let mut one_of = Vec::new();
        if let Some(one) = val.get("oneOf").and_then(|a| a.as_array()) {
            has_log = true;
            for item in one {
                one_of.push(LightSchema::parse(item)?);
            }
        }

        let not = match val.get("not") {
            Some(v) => {
                has_log = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };

        let conditional_if = match val.get("if") {
            Some(v) => {
                has_log = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };
        let conditional_then = match val.get("then") {
            Some(v) => {
                has_log = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };
        let conditional_else = match val.get("else") {
            Some(v) => {
                has_log = true;
                Some(Box::new(LightSchema::parse(v)?))
            }
            None => None,
        };

        let log_constraints = if has_log {
            Some(Box::new(LogicConstraints {
                any_of,
                all_of,
                one_of,
                not,
                conditional_if,
                conditional_then,
                conditional_else,
            }))
        } else {
            None
        };

        Ok(Self {
            types,
            dynamic_reference,
            reference,
            format,
            title,
            description,
            default,
            examples,
            enum_values,
            const_value,
            obj: obj_constraints,
            arr: arr_constraints,
            num: num_constraints,
            str: str_constraints,
            log: log_constraints,
        })
    }

    /// Creates an empty schema that validates anything.
    pub fn empty() -> Self {
        Self {
            types: alloc::vec![SchemaType::Any],
            dynamic_reference: None,
            reference: None,
            format: None,
            title: None,
            description: None,
            default: None,
            examples: None,
            enum_values: None,
            const_value: None,
            obj: None,
            arr: None,
            num: None,
            str: None,
            log: None,
        }
    }

    /// Validates a JSON payload against this schema.
    ///
    /// This is the primary public entrypoint for validation. It wraps the internal
    /// state tracking and returns a simple `Ok(())` on success, or a `Vec<ValidationError>` of
    /// error messages on failure.
    pub fn validate(
        &self,
        val: &Value,
        registry: Option<&SchemaRegistry>,
        options: Option<ValidationOptions>,
    ) -> ValidationOutput {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();
        let opts = options.unwrap_or_default();
        match self.validate_internal(val, registry, &opts, 0, &mut warnings) {
            Ok(_) => ValidationOutput {
                is_valid: true,
                errors: alloc::vec![],
                warnings,
            },
            Err(errs) => {
                errors.extend(errs);
                ValidationOutput {
                    is_valid: false,
                    errors,
                    warnings,
                }
            }
        }
    }

    pub fn validate_internal(
        &self,
        val: &Value,
        registry: Option<&SchemaRegistry>,
        options: &ValidationOptions,
        depth: usize,
        warnings: &mut Vec<ValidationError>,
    ) -> Result<EvaluationState, Vec<ValidationError>> {
        let stop_on_first_error = options.stop_on_first_error;
        let mut errors = Vec::new();

        macro_rules! check_early_stop {
            () => {
                if stop_on_first_error && !errors.is_empty() {
                    return Err(errors);
                }
            };
        }

        let mut state = EvaluationState::default();

        if depth > options.max_depth {
            return Err(alloc::vec![ValidationError::MaxDepthExceeded]);
        }

        // --- Step 1: Resolve references ---
        if let Some(r) = &self.dynamic_reference {
            if let Some(reg) = registry {
                if let Some(resolved) = reg.schemas.get(r) {
                    match resolved.validate_internal(val, registry, options, depth + 1, warnings) {
                        Ok(sub_state) => state.merge(&sub_state),
                        Err(sub_errs) => {
                            errors.extend(sub_errs);
                            check_early_stop!();
                        }
                    }
                } else {
                    errors.push(ValidationError::UnresolvedReference(r.clone()));
                    check_early_stop!();
                }
            } else {
                errors.push(ValidationError::UnresolvedReference(r.clone()));
                check_early_stop!();
            }
        }

        if let Some(ref_str) = &self.reference {
            if let Some(reg) = registry {
                if let Some(resolved_schema) = reg.schemas.get(ref_str) {
                    match resolved_schema.validate_internal(
                        val,
                        registry,
                        options,
                        depth + 1,
                        warnings,
                    ) {
                        Ok(sub_state) => state.merge(&sub_state),
                        Err(mut sub_errs) => {
                            errors.push(ValidationError::UnresolvedReference(ref_str.clone()));
                            check_early_stop!();
                            errors.append(&mut sub_errs);
                        }
                    }
                } else {
                    errors.push(ValidationError::UnresolvedReference(ref_str.clone()));
                    check_early_stop!();
                }
            } else {
                errors.push(ValidationError::UnresolvedReference(ref_str.clone()));
                check_early_stop!();
            }
        }

        // --- Step 2: Constant and Enum matching ---
        if let Some(enums) = &self.enum_values
            && !enums.contains(val)
        {
            errors.push(ValidationError::NotInEnum);
            check_early_stop!();
        }
        if let Some(c) = &self.const_value
            && c != val
        {
            errors.push(ValidationError::ConstMismatch);
            check_early_stop!();
        }

        // --- Step 3: Type validation ---
        let mut type_matched = false;
        if self.types.contains(&SchemaType::Any) || self.types.is_empty() {
            type_matched = true;
        } else {
            for ty in &self.types {
                match ty {
                    SchemaType::Object => {
                        if val.is_object() {
                            type_matched = true;
                        }
                    }
                    SchemaType::Array => {
                        if val.is_array() {
                            type_matched = true;
                        }
                    }
                    SchemaType::String => {
                        if val.is_string() {
                            type_matched = true;
                        }
                    }
                    SchemaType::Integer => {
                        if val.is_i64() || val.is_u64() {
                            type_matched = true;
                        }
                    }
                    SchemaType::Number => {
                        if val.is_number() {
                            type_matched = true;
                        }
                    }
                    SchemaType::Boolean => {
                        if val.is_boolean() {
                            type_matched = true;
                        }
                    }
                    SchemaType::Null => {
                        if val.is_null() {
                            type_matched = true;
                        }
                    }
                    SchemaType::Any => type_matched = true,
                }
                if type_matched {
                    break;
                }
            }
        }

        if !type_matched {
            errors.push(ValidationError::TypeMismatch(self.types[0].clone()));
            check_early_stop!();
            return Err(errors);
        }

        // --- Step 4: Object Constraints ---
        if let Some(obj_val) = val.as_object()
            && let Some(obj) = &self.obj
        {
            // Check property counts
            if let Some(min_p) = obj.min_properties
                && obj_val.len() < min_p
            {
                errors.push(ValidationError::MinProperties(min_p));
                check_early_stop!();
            }
            if let Some(max_p) = obj.max_properties
                && obj_val.len() > max_p
            {
                errors.push(ValidationError::MaxProperties(max_p));
                check_early_stop!();
            }

            for req in &obj.required {
                if !obj_val.contains_key(req) {
                    errors.push(ValidationError::MissingRequired(req.clone()));
                    check_early_stop!();
                }
            }

            for (dep_key, req_keys) in &obj.dependent_required {
                if obj_val.contains_key(dep_key) {
                    for req in req_keys {
                        if !obj_val.contains_key(req) {
                            errors.push(ValidationError::MissingDependentRequired {
                                req: req.clone(),
                                dep: dep_key.clone(),
                            });
                            check_early_stop!();
                        }
                    }
                }
            }

            for (dep_key, schema) in &obj.dependent_schemas {
                if obj_val.contains_key(dep_key) {
                    match schema.validate_internal(val, registry, options, depth + 1, warnings) {
                        Ok(sub_state) => state.merge(&sub_state),
                        Err(sub_errs) => {
                            errors.push(ValidationError::DependentSchemaFailed(dep_key.clone()));
                            check_early_stop!();
                            let wrapped: Vec<_> = sub_errs
                                .into_iter()
                                .map(|e| ValidationError::InProperty {
                                    key: dep_key.clone(),
                                    error: Box::new(e),
                                })
                                .collect();
                            errors.extend(wrapped);
                            check_early_stop!();
                        }
                    }
                }
            }

            for (k, v) in obj_val {
                if let Some(name_schema) = &obj.property_names
                    && let Err(sub_errs) = name_schema.validate_internal(
                        &Value::String(k.clone()),
                        registry,
                        options,
                        depth + 1,
                        warnings,
                    )
                {
                    errors.push(ValidationError::InvalidPropertyName(k.clone()));
                    check_early_stop!();
                    let wrapped: Vec<_> = sub_errs
                        .into_iter()
                        .map(|e| ValidationError::InProperty {
                            key: k.clone(),
                            error: Box::new(e),
                        })
                        .collect();
                    errors.extend(wrapped);
                    check_early_stop!();
                }

                let mut matched_properties = false;
                if let Some(prop_schema) = obj.properties.get(k) {
                    matched_properties = true;
                    match prop_schema.validate_internal(v, registry, options, depth + 1, warnings) {
                        Ok(_) => {
                            state.evaluated_properties.insert(k.clone());
                        }
                        Err(sub_errs) => {
                            let wrapped: Vec<_> = sub_errs
                                .into_iter()
                                .map(|e| ValidationError::InProperty {
                                    key: k.clone(),
                                    error: Box::new(e),
                                })
                                .collect();
                            errors.extend(wrapped);
                            check_early_stop!();
                        }
                    }
                }

                let mut matched_pattern = false;
                for (regex, pat_schema) in &obj.pattern_properties {
                    if regex.is_match(k) {
                        matched_pattern = true;
                        match pat_schema.validate_internal(
                            v,
                            registry,
                            options,
                            depth + 1,
                            warnings,
                        ) {
                            Ok(_) => {
                                state.evaluated_properties.insert(k.clone());
                            }
                            Err(sub_errs) => {
                                let wrapped: Vec<_> = sub_errs
                                    .into_iter()
                                    .map(|e| ValidationError::InProperty {
                                        key: k.clone(),
                                        error: Box::new(e),
                                    })
                                    .collect();
                                errors.extend(wrapped);
                                check_early_stop!();
                            }
                        }
                    }
                }

                if !matched_properties && !matched_pattern {
                    if let Some(add_schema) = &obj.additional_properties_schema {
                        match add_schema.validate_internal(
                            v,
                            registry,
                            options,
                            depth + 1,
                            warnings,
                        ) {
                            Ok(_) => {
                                state.evaluated_properties.insert(k.clone());
                            }
                            Err(sub_errs) => {
                                let wrapped: Vec<_> = sub_errs
                                    .into_iter()
                                    .map(|e| ValidationError::InProperty {
                                        key: k.clone(),
                                        error: Box::new(e),
                                    })
                                    .collect();
                                errors.extend(wrapped);
                                check_early_stop!();
                            }
                        }
                    } else if let Some(false) = obj.additional_properties_allowed {
                        errors.push(ValidationError::AdditionalPropertyNotAllowed(k.clone()));
                        check_early_stop!();
                    } else if let Some(true) = obj.additional_properties_allowed {
                        state.evaluated_properties.insert(k.clone());
                    }
                }
            }
        }

        // --- Step 5: Array Constraints ---
        if let Some(arr_val) = val.as_array()
            && let Some(arr) = &self.arr
        {
            // Check length boundaries
            if let Some(min_i) = arr.min_items
                && arr_val.len() < min_i
            {
                errors.push(ValidationError::MinItems(min_i));
                check_early_stop!();
            }
            if let Some(max_i) = arr.max_items
                && arr_val.len() > max_i
            {
                errors.push(ValidationError::MaxItems(max_i));
                check_early_stop!();
            }
            if let Some(true) = arr.unique_items {
                let mut unique = true;
                for i in 0..arr_val.len() {
                    for j in (i + 1)..arr_val.len() {
                        if arr_val[i] == arr_val[j] {
                            unique = false;
                            break;
                        }
                    }
                }
                if !unique {
                    errors.push(ValidationError::NotUnique);
                    check_early_stop!();
                }
            }

            let mut validated_indices = 0;
            if let Some(prefixes) = &arr.prefix_items {
                for (idx, (schema, item)) in prefixes.iter().zip(arr_val.iter()).enumerate() {
                    match schema.validate_internal(item, registry, options, depth + 1, warnings) {
                        Ok(_) => {
                            state.evaluated_items.insert(idx);
                        }
                        Err(sub_errs) => {
                            errors.push(ValidationError::PrefixItemFailed(idx));
                            check_early_stop!();
                            let wrapped: Vec<_> = sub_errs
                                .into_iter()
                                .map(|e| ValidationError::InIndex {
                                    index: idx,
                                    error: Box::new(e),
                                })
                                .collect();
                            errors.extend(wrapped);
                            check_early_stop!();
                        }
                    }
                    validated_indices += 1;
                }
            }

            if let Some(item_schema) = &arr.items {
                for (idx, item) in arr_val.iter().enumerate().skip(validated_indices) {
                    match item_schema.validate_internal(
                        item,
                        registry,
                        options,
                        depth + 1,
                        warnings,
                    ) {
                        Ok(_) => {
                            state.evaluated_items.insert(idx);
                        }
                        Err(sub_errs) => {
                            errors.push(ValidationError::ItemFailed(idx));
                            check_early_stop!();
                            let wrapped: Vec<_> = sub_errs
                                .into_iter()
                                .map(|e| ValidationError::InIndex {
                                    index: idx,
                                    error: Box::new(e),
                                })
                                .collect();
                            errors.extend(wrapped);
                            check_early_stop!();
                        }
                    }
                }
            }

            if let Some(contains_schema) = &arr.contains {
                let mut contains_count = 0;
                for (idx, item) in arr_val.iter().enumerate() {
                    if contains_schema
                        .validate_internal(item, registry, options, depth + 1, warnings)
                        .is_ok()
                    {
                        contains_count += 1;
                        state.evaluated_items.insert(idx);
                    }
                }
                if contains_count == 0 && arr.min_contains.unwrap_or(1) > 0 {
                    errors.push(ValidationError::ContainsNoMatch);
                    check_early_stop!();
                }
                if let Some(min_c) = arr.min_contains
                    && contains_count < min_c
                {
                    errors.push(ValidationError::MinContains(min_c));
                    check_early_stop!();
                }
                if let Some(max_c) = arr.max_contains
                    && contains_count > max_c
                {
                    errors.push(ValidationError::MaxContains(max_c));
                    check_early_stop!();
                }
            }
        }

        // --- Step 6: String Constraints ---
        if let Some(s) = val.as_str() {
            if let Some(str_c) = &self.str {
                let len = s.chars().count();
                // Check bounds
                if let Some(min_l) = str_c.min_length
                    && len < min_l
                {
                    errors.push(ValidationError::MinLength(min_l));
                    check_early_stop!();
                }
                if let Some(max_l) = str_c.max_length
                    && len > max_l
                {
                    errors.push(ValidationError::MaxLength(max_l));
                    check_early_stop!();
                }
                if let Some(pat) = &str_c.pattern
                    && !pat.is_match(s)
                {
                    errors.push(ValidationError::PatternMismatch);
                    check_early_stop!();
                }
            }

            if let Some(fmt) = &self.format {
                let mut is_valid = true;
                match fmt {
                    SchemaFormat::Ipv4 => {
                        if Ipv4Addr::from_str(s).is_err() {
                            is_valid = false;
                        }
                    }
                    SchemaFormat::Ipv6 => {
                        if Ipv6Addr::from_str(s).is_err() {
                            is_valid = false;
                        }
                    }
                    SchemaFormat::Uri => {
                        if Url::parse(s).is_err() {
                            is_valid = false;
                        }
                    }
                    SchemaFormat::Email(email_regex) => {
                        if !email_regex.is_match(s) {
                            is_valid = false;
                        }
                    }
                    SchemaFormat::DateTime => {
                        if iso8601::datetime(s).is_err() {
                            is_valid = false;
                        }
                    }
                }

                if !is_valid {
                    let format_name = match fmt {
                        SchemaFormat::Ipv4 => "ipv4",
                        SchemaFormat::Ipv6 => "ipv6",
                        SchemaFormat::Uri => "uri",
                        SchemaFormat::Email(_) => "email",
                        SchemaFormat::DateTime => "date-time",
                    };
                    let err = ValidationError::InvalidFormat(format_name.to_string());
                    if options.format_assertions {
                        errors.push(err);
                        check_early_stop!();
                    } else {
                        warnings.push(err);
                    }
                }
            }
        }

        // --- Step 7: Numeric Constraints ---
        if let Some(n) = val.as_f64()
            && let Some(num) = &self.num
        {
            if let Some(min_v) = num.minimum
                && n < min_v
            {
                errors.push(ValidationError::Minimum(min_v));
                check_early_stop!();
            }
            if let Some(max_v) = num.maximum
                && n > max_v
            {
                errors.push(ValidationError::Maximum(max_v));
                check_early_stop!();
            }
            if let Some(ex_min) = num.exclusive_minimum
                && n <= ex_min
            {
                errors.push(ValidationError::ExclusiveMinimum(ex_min));
                check_early_stop!();
            }
            if let Some(ex_max) = num.exclusive_maximum
                && n >= ex_max
            {
                errors.push(ValidationError::ExclusiveMaximum(ex_max));
                check_early_stop!();
            }
            if let Some(mult) = num.multiple_of
                && mult > 0.0
            {
                // Use a safer tolerance to prevent precision errors
                let rem = n % mult;
                if rem.abs() > 1e-10 && (mult - rem).abs() > 1e-10 {
                    errors.push(ValidationError::MultipleOf(mult));
                    check_early_stop!();
                }
            }
        }

        // --- Step 8: Logical Composition (anyOf, allOf, oneOf, not) ---
        if let Some(log) = &self.log {
            if !log.any_of.is_empty() {
                let mut any_valid = false;
                for branch in &log.any_of {
                    if let Ok(sub_state) =
                        branch.validate_internal(val, registry, options, depth + 1, warnings)
                    {
                        state.merge(&sub_state);
                        any_valid = true;
                        break;
                    }
                }
                if !any_valid {
                    errors.push(ValidationError::AnyOfFailed);
                    check_early_stop!();
                }
            }
            if !log.all_of.is_empty() {
                for branch in &log.all_of {
                    match branch.validate_internal(val, registry, options, depth + 1, warnings) {
                        Ok(sub_state) => state.merge(&sub_state),
                        Err(mut sub_errs) => {
                            errors.push(ValidationError::AllOfFailed);
                            check_early_stop!();
                            errors.append(&mut sub_errs);
                        }
                    }
                }
            }
            if !log.one_of.is_empty() {
                let mut matches = 0;
                let mut best_state = EvaluationState::default();
                for branch in &log.one_of {
                    if let Ok(sub_state) =
                        branch.validate_internal(val, registry, options, depth + 1, warnings)
                    {
                        matches += 1;
                        best_state = sub_state;
                    }
                }
                if matches != 1 {
                    errors.push(ValidationError::OneOfMatches(matches));
                    check_early_stop!();
                } else {
                    state.merge(&best_state);
                }
            }
            if let Some(not_schema) = &log.not
                && not_schema
                    .validate_internal(val, registry, options, depth + 1, warnings)
                    .is_ok()
            {
                errors.push(ValidationError::NotFailed);
                check_early_stop!();
            }

            if let Some(cond_if) = &log.conditional_if {
                let if_res = cond_if.validate_internal(val, registry, options, depth + 1, warnings);
                if let Ok(if_state) = if_res {
                    state.merge(&if_state);
                    if let Some(cond_then) = &log.conditional_then {
                        match cond_then.validate_internal(
                            val,
                            registry,
                            options,
                            depth + 1,
                            warnings,
                        ) {
                            Ok(sub_state) => state.merge(&sub_state),
                            Err(mut sub_errs) => {
                                errors.push(ValidationError::ThenFailed);
                                check_early_stop!();
                                errors.append(&mut sub_errs);
                            }
                        }
                    }
                } else if let Some(cond_else) = &log.conditional_else {
                    match cond_else.validate_internal(val, registry, options, depth + 1, warnings) {
                        Ok(sub_state) => state.merge(&sub_state),
                        Err(mut sub_errs) => {
                            errors.push(ValidationError::ElseFailed);
                            check_early_stop!();
                            errors.append(&mut sub_errs);
                        }
                    }
                }
            }
        }

        // --- Post-Evaluation Unevaluated Checks ---
        if let Some(obj_val) = val.as_object()
            && let Some(obj) = &self.obj
            && let Some(uneval) = &obj.unevaluated_properties
        {
            for (k, v) in obj_val {
                if !state.evaluated_properties.contains(k) {
                    match uneval.validate_internal(v, registry, options, depth + 1, warnings) {
                        Ok(_) => {
                            state.evaluated_properties.insert(k.clone());
                        }
                        Err(sub_errs) => {
                            errors.push(ValidationError::UnevaluatedPropertyFailed(k.clone()));
                            check_early_stop!();
                            let wrapped: Vec<_> = sub_errs
                                .into_iter()
                                .map(|e| ValidationError::InProperty {
                                    key: k.clone(),
                                    error: Box::new(e),
                                })
                                .collect();
                            errors.extend(wrapped);
                            check_early_stop!();
                        }
                    }
                }
            }
        }
        if let Some(arr_val) = val.as_array()
            && let Some(arr) = &self.arr
            && let Some(uneval) = &arr.unevaluated_items
        {
            for (idx, item) in arr_val.iter().enumerate() {
                if !state.evaluated_items.contains(&idx) {
                    match uneval.validate_internal(item, registry, options, depth + 1, warnings) {
                        Ok(_) => {
                            state.evaluated_items.insert(idx);
                        }
                        Err(sub_errs) => {
                            errors.push(ValidationError::UnevaluatedItemFailed(idx));
                            check_early_stop!();
                            let wrapped: Vec<_> = sub_errs
                                .into_iter()
                                .map(|e| ValidationError::InIndex {
                                    index: idx,
                                    error: Box::new(e),
                                })
                                .collect();
                            errors.extend(wrapped);
                            check_early_stop!();
                        }
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(state)
        } else {
            Err(errors)
        }
    }
}
