use anyhow::{Result, anyhow};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::Path;

const PROD_API_SPEC_URL: &str = "https://www.deribit.com/static/deribit_api_v2.json";
const TESTNET_API_SPEC_URL: &str = "https://test.deribit.com/static/deribit_api_v2.json";

#[derive(Debug)]
struct ApiMethod {
    name: String,
    params: Vec<Parameter>,
    response_type: TokenStream,
}

#[derive(Debug)]
struct Parameter {
    name: String,
    param_type: TokenStream,
    required: bool,
}

struct DeribitApiGen {
    spec: Value,
    generated_code: TokenStream,
    generated_types: HashSet<String>,
    ref_names: HashMap<String, String>,
}

impl DeribitApiGen {
    fn new(spec_url: &str) -> Result<Self> {
        // Download API spec
        let spec = Self::download_api_spec(spec_url)?;
        let generated_code = TokenStream::new();
        let generated_types = HashSet::new();
        let ref_names = HashMap::new();
        let mut api_gen = Self {
            spec,
            generated_code,
            generated_types,
            ref_names,
        };

        // Generate all methods and types from the spec
        api_gen.generate_ref_names();
        api_gen.generate_methods()?;
        api_gen.generate_subscription_code();
        Ok(api_gen)
    }

    fn generate_ref_names(&mut self) {
        let components = self.spec.get("components").unwrap();
        let schemas = components
            .get("schemas")
            .and_then(|s| s.as_object())
            .unwrap();
        let types = schemas.get("types").and_then(|t| t.as_object()).unwrap();
        let parameters = components
            .get("parameters")
            .and_then(|p| p.as_object())
            .unwrap();
        let mut seen_names = HashSet::new();
        for name in types.keys() {
            seen_names.insert(name.clone());
            self.ref_names
                .insert(format!("#/components/schemas/types/{}", name), name.clone());
        }
        for name in schemas.keys() {
            if seen_names.insert(name.clone()) {
                self.ref_names
                    .insert(format!("#/components/schemas/{}", name), name.clone());
            } else {
                self.ref_names.insert(
                    format!("#/components/schemas/{}", name),
                    format!("{}_schema", name),
                );
            }
        }
        for name in parameters.keys() {
            if seen_names.insert(name.clone()) {
                self.ref_names
                    .insert(format!("#/components/parameters/{}", name), name.clone());
            } else {
                self.ref_names.insert(
                    format!("#/components/parameters/{}", name),
                    format!("{}_param", name),
                );
            }
        }
    }

    fn download_api_spec(spec_url: &str) -> Result<Value> {
        // Support local file paths in addition to URLs to make development easier
        if spec_url.starts_with("http://") || spec_url.starts_with("https://") {
            let response = reqwest::blocking::get(spec_url)
                .map_err(|e| anyhow!("Failed to download API spec: {}", e))?;
            let spec: Value = response
                .json()
                .map_err(|e| anyhow!("Failed to parse API spec: {}", e))?;
            Ok(spec)
        } else {
            let content = fs::read_to_string(spec_url)
                .map_err(|e| anyhow!("Failed to read API spec file '{}': {}", spec_url, e))?;
            let spec: Value = serde_json::from_str(&content).map_err(|e| {
                anyhow!(
                    "Failed to parse API spec JSON from file '{}': {}",
                    spec_url,
                    e
                )
            })?;
            Ok(spec)
        }
    }

    fn extract_methods(&mut self) -> Result<Vec<ApiMethod>> {
        let paths = self
            .spec
            .get("paths")
            .and_then(|p| p.as_object())
            .ok_or_else(|| anyhow!("No paths found in API spec"))?
            .clone();

        // for (path, path_spec) in paths {
        let mut methods: Vec<ApiMethod> = paths
            .iter()
            .filter_map(|(path, path_spec)| {
                // Remove leading slash
                let method_name = path.trim_start_matches('/');

                let method_spec = path_spec.get("get")?;

                let params = self.extract_parameters(method_name, method_spec);
                let response_type = self.extract_response_type(method_name, method_spec);

                Some(ApiMethod {
                    name: method_name.to_string(),
                    params,
                    response_type,
                })
            })
            .collect();

        // Sort methods for consistent output
        methods.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(methods)
    }

    fn extract_response_type(&mut self, method_name: &str, method_spec: &Value) -> TokenStream {
        get_deep_value(
            &vec!["responses", "200", "content", "application/json", "schema"],
            method_spec,
        )
        .and_then(|v| {
            let schema_obj = v.as_object()?;

            let (type_name, expanded_schema) = self
                .expand_ref(schema_obj)
                .unwrap_or_else(|| (format!("{}_response", method_name), schema_obj.clone()));

            // Responses use allOf: [ base_message, { properties: { result: <schema> } } ]
            expanded_schema
                .get("allOf")?
                .as_array()?
                .iter()
                .find_map(|item| get_deep_value(&vec!["properties", "result"], item)?.as_object())
                .map(|schema| self.determine_type(&type_name, schema))
        })
        // Default to untyped value if anything is missing
        .unwrap_or_else(|| quote! { serde_json::Value })
    }

    fn extract_parameters(&mut self, method_name: &str, method_spec: &Value) -> Vec<Parameter> {
        method_spec
            .get("parameters")
            .and_then(|p| p.as_array())
            .map(|params| {
                params
                    .iter()
                    .filter_map(|param| {
                        let param_obj = param.as_object()?;
                        let (type_name, param_obj) = self.expand_ref(param_obj).or_else(|| {
                            let param_name = param_obj.get("name")?.as_str()?;
                            Some((format!("{}_{}", method_name, param_name), param_obj.clone()))
                        })?;
                        let param_name = param_obj.get("name")?.as_str()?;
                        let required = param_obj
                            .get("required")
                            .and_then(|r| r.as_bool())
                            .unwrap_or(false);
                        let schema = param_obj.get("schema")?.as_object()?;
                        let param_type = self.determine_type(&type_name, &schema);

                        Some(Parameter {
                            name: param_name.to_string(),
                            param_type,
                            required,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn resolve_ref(&mut self, ref_path: &str) -> Option<(String, Map<String, Value>)> {
        let ref_parts: Vec<&str> = ref_path.strip_prefix("#/")?.split('/').collect();
        get_deep_value(&ref_parts, &self.spec)?
            .as_object()
            .map(|r| {
                let name = ref_parts.last().unwrap().to_string();
                let name = self.ref_names.get(ref_path).unwrap_or(&name);
                (name.clone(), r.clone())
            })
    }

    fn expand_ref(&mut self, object: &Map<String, Value>) -> Option<(String, Map<String, Value>)> {
        let ref_path = object.get("$ref")?.as_str()?;
        self.resolve_ref(ref_path).map(|(name, mut ref_obj)| {
            let mut object = object.clone();
            object.remove("$ref");
            ref_obj.extend(object);
            self.expand_ref(&ref_obj).unwrap_or((name, ref_obj))
        })
    }

    fn determine_type(&mut self, name: &str, schema: &Map<String, Value>) -> TokenStream {
        let (type_name, schema) = self
            .expand_ref(schema)
            .unwrap_or_else(|| (name.to_string(), schema.clone()));

        if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
            let schema =
                all_of
                    .iter()
                    .filter_map(|v| v.as_object())
                    .fold(Map::new(), |mut acc, obj| {
                        let (_, schema) = self
                            .expand_ref(obj)
                            .unwrap_or_else(|| ("".to_string(), obj.clone()));
                        for (key, value) in schema {
                            match key.as_str() {
                                "properties" => {
                                    let properties = acc
                                        .get("properties")
                                        .and_then(|v| v.as_object())
                                        .and_then(|properties| {
                                            value.as_object().map(|p| {
                                                let mut properties = properties.clone();
                                                properties.extend(p.clone().into_iter());
                                                Value::Object(properties)
                                            })
                                        })
                                        .unwrap_or(value);
                                    acc.insert(key, properties);
                                }
                                "required" => {
                                    let required = acc
                                        .get("required")
                                        .and_then(|v| v.as_array())
                                        .and_then(|required| {
                                            value.as_array().map(|r| {
                                                let mut required = required.clone();
                                                required.extend(r.clone().into_iter());
                                                Value::Array(required)
                                            })
                                        })
                                        .unwrap_or(value);
                                    acc.insert(key, required);
                                }
                                _ => {
                                    acc.insert(key, value);
                                }
                            }
                        }
                        acc
                    });
            return self.determine_type(&type_name, &schema);
        }

        let schema_type = schema.get("type").and_then(|t| t.as_str()).or_else(|| {
            if schema.contains_key("properties") {
                Some("object")
            } else if schema.contains_key("items") {
                Some("array")
            } else {
                None
            }
        });

        match schema_type {
            Some("string") => {
                if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
                    let enum_name = format_ident!("{}", to_valid_pascal_case(&type_name));

                    if self.generated_types.insert(enum_name.to_string()) {
                        let enum_values = enum_values
                            .iter()
                            .map(|v| {
                                let value = v
                                    .as_str()
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| v.to_string());
                                let value_name = format_ident!("{}", to_valid_pascal_case(&value));
                                quote! {
                                    #[serde(rename = #value)]
                                    #value_name
                                }
                            })
                            .collect::<Vec<_>>();

                        self.generated_code.extend(quote! {
                            #[derive(Debug, Default, Clone, Serialize, Deserialize)]
                            pub enum #enum_name {
                                #[default]
                                #(#enum_values),*
                            }
                        });
                    }
                    quote! { #enum_name }
                } else {
                    quote! { String }
                }
            }
            Some("integer") => quote! { i64 },
            Some("number") => quote! { f64 },
            Some("boolean") => quote! { bool },
            Some("array") => match schema.get("items") {
                Some(Value::Object(items_schema)) => {
                    let item_type = self.determine_type(&type_name, items_schema);
                    quote! { Vec<#item_type> }
                }
                Some(Value::Array(items)) => {
                    let item_types = items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| {
                            let item_schema = item.as_object().unwrap();
                            let item_type_name = if let Some(description) =
                                item_schema.get("description").and_then(|d| d.as_str())
                            {
                                format!("{}_{}", type_name, description)
                            } else {
                                format!("{}_{}", type_name, i)
                            };
                            self.determine_type(&item_type_name, item_schema)
                        })
                        .collect::<Vec<_>>();
                    quote! { (#(#item_types),*) }
                }
                _ => quote! { Vec<Value> },
            },
            Some("object") => {
                if let Some(properties) = schema.get("properties") {
                    if let Some(property_type) = properties.get("$value").and_then(|v| {
                        let value = v.as_object()?;
                        let property_type_name =
                            if let Some(name) = value.get("name").and_then(|name| name.as_str()) {
                                format!("{}_{}", &type_name, name)
                            } else {
                                type_name.clone()
                            };
                        Some(
                            self.determine_type(
                                &property_type_name,
                                value.get("schema")?.as_object()?,
                            ),
                        )
                    }) {
                        return quote! { std::collections::HashMap<String, #property_type> };
                    }

                    let struct_name = format_ident!("{}", to_valid_pascal_case(&type_name));

                    if self.generated_types.insert(struct_name.to_string()) {
                        let required_properties = schema
                            .get("required")
                            .and_then(|r| r.as_array())
                            .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>())
                            .unwrap_or_default();
                        let properties = if let Some(properties) = properties.as_array() {
                            properties
                                .iter()
                                .filter_map(|property| {
                                    let property = property.as_object()?;
                                    let (property_type_name, property) =
                                        self.expand_ref(property).or_else(|| {
                                            let key = property.get("name")?.as_str()?;
                                            Some((
                                                format!("{}_{}", type_name, key),
                                                property.clone(),
                                            ))
                                        })?;
                                    let key = property.get("name")?.as_str()?;
                                    let required = property
                                        .get("required")
                                        .and_then(|r| r.as_bool())
                                        .unwrap_or(false);
                                    let property_type = self.determine_type(
                                        &property_type_name,
                                        property.get("schema")?.as_object()?,
                                    );
                                    Some(field_tokens(
                                        key,
                                        &property_type,
                                        required_properties.contains(&key) || required,
                                    ))
                                })
                                .collect::<Vec<_>>()
                        } else {
                            let mut properties_tokens = vec![];
                            for (key, value) in properties.as_object().unwrap() {
                                let property_type_name = format!("{}_{}", type_name, key);
                                let property_type = self.determine_type(
                                    &property_type_name,
                                    value.as_object().unwrap(),
                                );
                                if key.starts_with('{') && key.ends_with('}') {
                                    self.generated_types.remove(&struct_name.to_string());
                                    return quote! { std::collections::HashMap<String, #property_type> };
                                }
                                properties_tokens.push(field_tokens(
                                    key,
                                    &property_type,
                                    required_properties.contains(&key.as_str()),
                                ));
                            }
                            properties_tokens
                        };

                        self.generated_code.extend(quote! {
                            #[derive(Debug, Default, Clone, Serialize, Deserialize)]
                            pub struct #struct_name {
                                #(#properties),*
                            }
                        });
                    }
                    quote! { #struct_name }
                } else {
                    quote! { std::collections::HashMap<String, Value> }
                }
            }
            _ => quote! { Value },
        }
    }

    fn generate_methods(&mut self) -> Result<()> {
        for method in self.extract_methods()? {
            let struct_name = format_ident!("{}Request", to_valid_pascal_case(&method.name));
            let method_name = &method.name;
            let response_type = &method.response_type;

            // Generate fields
            let fields = method
                .params
                .iter()
                .map(|param| field_tokens(&param.name, &param.param_type, param.required))
                .collect::<Vec<_>>();

            self.generated_code.extend(quote! {
                #[derive(Debug, Default, Clone, Serialize, Deserialize)]
                pub struct #struct_name {
                    #(#fields),*
                }

                impl crate::ApiRequest for #struct_name {
                    type Response = #response_type;
                    fn method_name(&self) -> &'static str {
                        #method_name
                    }
                }
            });
        }
        Ok(())
    }

    fn get_client_code(&self) -> String {
        // Convert TokenStream to syn::File for prettyplease
        if let Ok(file) = syn::parse2::<syn::File>(self.generated_code.clone()) {
            // Format using prettyplease
            prettyplease::unparse(&file)
        } else {
            eprintln!("Warning: Failed to parse generated code for formatting");
            self.generated_code.to_string()
        }
    }

    fn generate_subscription_code(&mut self) {
        // Parse x-subscriptions to generate typed subscription channels and their data types
        let Some(subscriptions) =
            get_deep_value(&vec!["components", "x-subscriptions"], &self.spec)
                .and_then(|v| v.as_object())
                .cloned()
        else {
            return;
        };

        for (channel_key, channel_spec) in &subscriptions {
            let channel_name = channel_key
                .replace(".{interval}", "")
                .replace('.', "_")
                .replace('{', "")
                .replace('}', "");

            // Collect parameters (if any)
            let params_vec = self.extract_parameters(&channel_name, channel_spec);

            // Determine notification data type
            let notification_type = get_deep_value(&vec!["notifications", "schema"], channel_spec)
                .and_then(|v| v.as_object())
                .map(|schema| self.determine_type(&channel_name, schema))
                .unwrap_or_else(|| quote! { serde_json::Value });

            // Build struct name from channel key
            let channel_struct_name =
                format_ident!("{}Channel", to_valid_pascal_case(&channel_name));

            // Build struct fields
            let fields_tokens = params_vec
                .iter()
                .map(|p| field_tokens(&p.name, &p.param_type, true))
                .collect::<Vec<_>>();

            // Build channel string assembly code from pattern
            // Split by '.' and for each part, if it is a placeholder like {name}, replace with value serialization
            let join_segments = channel_key
                .split('.')
                .map(|part| {
                    if part.starts_with('{') && part.ends_with('}') {
                        let param_name = &part[1..part.len() - 1];
                        let ident = format_ident!("{}", to_valid_snake_case(param_name));
                        quote! { crate::sub_param_to_string(&self.#ident) }
                    } else {
                        let lit = part.to_string();
                        quote! { #lit.to_string() }
                    }
                })
                .collect::<Vec<_>>();

            self.generated_code.extend(quote! {
                #[derive(Debug, Clone, Serialize, Deserialize)]
                pub struct #channel_struct_name {
                    #(#fields_tokens),*
                }

                impl crate::Subscription for #channel_struct_name {
                    type Data = #notification_type;
                    fn channel_string(&self) -> String {
                        vec![ #(#join_segments),* ].join(".")
                    }
                }
            });
        }
    }
}

fn get_deep_value<'a>(path: &Vec<&str>, value: &'a Value) -> Option<&'a Value> {
    let mut value = value;
    for key in path {
        value = value.get(key)?;
    }
    Some(value)
}

fn field_tokens(name: &str, field_type: &TokenStream, required: bool) -> TokenStream {
    let mut tokens = TokenStream::new();
    let field_name = format_ident!("{}", to_valid_snake_case(name));

    if field_name.to_string() != name {
        tokens.extend(quote! {
            #[serde(rename = #name)]
        });
    }

    if required {
        tokens.extend(quote! {
            #[serde(default)]
            pub #field_name: #field_type
        });
    } else {
        tokens.extend(quote! {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub #field_name: Option<#field_type>
        });
    }

    tokens
}

fn to_pascal_case(s: &str) -> String {
    let result = s
        .split('/')
        .map(|part| {
            part.split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            first.to_uppercase().collect::<String>()
                                + &chars.as_str().to_lowercase()
                        }
                    }
                })
                .collect::<String>()
        })
        .collect::<String>();
    if result.chars().next().map_or(false, |c| c.is_ascii_digit()) {
        format!("_{}", result)
    } else {
        result
    }
}

fn to_snake_case(s: &str) -> String {
    let mut result = String::new();

    if s.chars()
        .all(|c| c.is_uppercase() || !c.is_ascii_alphabetic())
    {
        return s.to_lowercase();
    }

    for ch in s.chars() {
        if ch.is_uppercase() {
            if !result.is_empty() {
                result.push('_');
            }
            result.push(ch.to_lowercase().next().unwrap());
        } else {
            result.push(ch);
        }
    }

    result
}

fn escape_rust_keyword(s: &str) -> String {
    // List of Rust keywords that need to be escaped
    let keywords = [
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do",
        "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
    ];

    if keywords.contains(&s) {
        format!("r#{s}")
    } else {
        s.to_string()
    }
}

fn sanitize_ident(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return "_".to_string();
    }
    if out.chars().next().unwrap().is_ascii_digit() {
        out.insert(0, '_');
    }
    out
}

fn to_valid_pascal_case(s: &str) -> String {
    sanitize_ident(&to_pascal_case(s))
}

fn to_valid_snake_case(s: &str) -> String {
    let sanitized = sanitize_ident(&to_snake_case(s));
    escape_rust_keyword(&sanitized)
}

fn read_manifest_spec_url() -> Option<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let cargo_toml_path = Path::new(&manifest_dir).join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml_path).ok()?;
    let value: toml::Value = toml::from_str(&content).ok()?;

    value
        .get("package")?
        .get("metadata")?
        .get("deribit")
        .and_then(|d| d.get("api_spec_url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // Rebuild if manifest changes (we read an optional spec URL from it)
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        println!(
            "cargo:rerun-if-changed={}",
            Path::new(&manifest_dir).join("Cargo.toml").display()
        );
    }
    // Feature flags are passed through env as CARGO_FEATURE_<FEATURE_NAME>
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_TESTNET");

    let out_dir = env::var("OUT_DIR").unwrap();
    let prod_spec_url = read_manifest_spec_url().unwrap_or_else(|| PROD_API_SPEC_URL.to_string());
    let prod_gen = DeribitApiGen::new(&prod_spec_url).unwrap();
    let dest_prod = Path::new(&out_dir).join("deribit_client_prod.rs");
    fs::write(&dest_prod, prod_gen.get_client_code()).unwrap();
    // Env var for discoverability (points to prod by convention)
    println!(
        "cargo:rustc-env=GENERATED_DERIBIT_CLIENT_PATH={}",
        dest_prod.display()
    );

    if env::var("CARGO_FEATURE_TESTNET").is_ok() {
        let testnet_gen = DeribitApiGen::new(TESTNET_API_SPEC_URL).unwrap();
        let dest_testnet = Path::new(&out_dir).join("deribit_client_testnet.rs");
        fs::write(&dest_testnet, testnet_gen.get_client_code()).unwrap();
    }
}
