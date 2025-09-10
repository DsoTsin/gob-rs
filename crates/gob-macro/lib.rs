use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, DeriveInput, Meta, Token, Data, Fields};
use darling::{FromMeta, FromAttributes, ast::NestedMeta};

#[derive(Debug, FromMeta)]
struct GobArgs {
    #[darling(default)]
    id: Option<i64>,
    #[darling(default)]
    interpret_as: Option<String>,
    // type alias name
    #[darling(default)]
    name: Option<String>,
}

impl GobArgs {
    fn parse_map_types(&self) -> Option<(String, String)> {
        let interpret_as = self.interpret_as.as_ref()?;
        
        // Parse "map[KeyType]ValueType"
        if !interpret_as.starts_with("map[") {
            return None;
        }
        
        let rest = &interpret_as[4..]; // Skip "map["
        let bracket_pos = rest.find(']')?;
        let key_type = rest[..bracket_pos].to_string();
        let value_type = rest[bracket_pos + 1..].to_string();
        
        Some((key_type, value_type))
    }
}

#[derive(Debug, FromAttributes)]
#[darling(attributes(gob))]
struct GobFieldArgs {
    #[darling(default)]
    name: Option<String>,
}

#[proc_macro_attribute]
#[allow(non_snake_case)]
pub fn Gob(args: TokenStream, input: TokenStream) -> TokenStream {
    let attr_args = parse_macro_input!(args with Punctuated::<Meta, Token![,]>::parse_terminated);
    let attr_args: Vec<NestedMeta> = attr_args.into_iter().map(NestedMeta::Meta).collect();
    let mut item = parse_macro_input!(input as DeriveInput);

    let gob_args = match GobArgs::from_list(&attr_args) {
        Ok(v) => v,
        Err(e) => {
            return TokenStream::from(e.write_errors());
        }
    };

    let mut encode_fields = Vec::new();
    let mut decode_fields = Vec::new();
    let mut map_decode_fields = Vec::new();
    let mut map_encode_fields = Vec::new(); // For map-based encoding (fields sorted by key)
    
    if let Data::Struct(ref mut data) = item.data {
        if let Fields::Named(ref mut fields) = data.fields {
            // Collect fields to sort them for map encoding
            struct FieldInfo {
                name: String,
                ident: syn::Ident,
            }
            let mut sorted_fields = Vec::new();

            for (index, field) in fields.named.iter_mut().enumerate() {
                let (gob_attrs, other_attrs): (Vec<_>, Vec<_>) = field.attrs.iter().cloned().partition(|attr| {
                    attr.path().is_ident("gob")
                });
                
                field.attrs = other_attrs;

                // Default field name is the struct field name
                let field_ident = field.ident.as_ref().unwrap();
                let mut field_name_str = field_ident.to_string(); 
                
                // Check if we have a custom name
                if !gob_attrs.is_empty() {
                    if let Ok(args) = GobFieldArgs::from_attributes(&gob_attrs) {
                         if let Some(name) = args.name {
                             field_name_str = name;
                         }
                    } else if let Err(e) = GobFieldArgs::from_attributes(&gob_attrs) {
                        return TokenStream::from(e.write_errors());
                    }
                }
                
                // Collect for sorted map encoding
                sorted_fields.push(FieldInfo {
                    name: field_name_str.clone(),
                    ident: field_ident.clone(),
                });

                // Generate encode logic for this field
                let field_num = (index + 1) as u64;
                
                encode_fields.push(quote! {
                    // Field delta: current field num - last field num. 
                    encoder.write_uint(#field_num - last_field_num)?; 
                    last_field_num = #field_num;
                    
                    // Encode value
                    gobx::GobEncodable::encode(&self.#field_ident, encoder)?;
                });

                // Generate decode logic for this field (Struct mode)
                let field_num_i64 = field_num as i64;
                decode_fields.push(quote! {
                     #field_num_i64 => {
                         let val = gobx::GobDecodable::decode(decoder)?;
                         result.#field_ident = val;
                     }
                });
                
                // Generate decode logic for this field (Map mode)
                map_decode_fields.push(quote! {
                    #field_name_str => {
                        if let Ok(v) = std::convert::TryInto::try_into(value_val.clone()) {
                             result.#field_ident = v;
                        } else {
                            // Try harder? e.g. Uint to Int cast
                             // For now, simple TryInto.
                        }
                    }
                });
            }
            
            // Sort fields by name for consistent map encoding
            sorted_fields.sort_by(|a, b| a.name.cmp(&b.name));
            
            for f in sorted_fields {
                let name = f.name;
                let ident = f.ident;
                
                // Generate map encoding that encodes both key and value as interfaces
                // Key is always a string (the field name)
                // Value depends on map_types - if interface{}, encode with type info
                
                map_encode_fields.push(quote! {
                    // Encode key as interface (string type)
                    encoder.write_string(#name)?; // Type name for string
                    encoder.write_int(6)?; // Type ID 6 = string
                    
                    // Encode the key string value (length + bytes)
                    let key_bytes = #name.as_bytes();
                    encoder.write_uint(key_bytes.len() as u64)?;
                    encoder.write_all(key_bytes)?;
                    
                    // Encode value as interface
                    // We need to determine the type name and ID at runtime
                    // For now, we'll use GobEncodable trait methods
                    gobx::encode_as_interface(&self.#ident, encoder)?;
                });
            }
        }
    }
    
    // Check if we need to interpret as map
    let interpret_as_map = gob_args.interpret_as.as_ref().map_or(false, |s| s.starts_with("map["));
    let map_types = gob_args.parse_map_types();
    
    let encode_impl = if interpret_as_map {
        let count_lit = proc_macro2::Literal::u64_unsuffixed(map_encode_fields.len() as u64);
        
        // Check if we need interface encoding
        let value_is_interface = map_types.as_ref()
            .map(|(_, v)| v == "interface{}")
            .unwrap_or(false);
        
        if value_is_interface {
            // For map[K]interface{}, encode each value as interface
            quote! {
                encoder.write_uint(#count_lit)?;
                
                #(#map_encode_fields)*
                Ok(())
            }
        } else {
            // Simple map encoding
            quote! {
                encoder.write_uint(#count_lit)?;
                
                #(#map_encode_fields)*
                Ok(())
            }
        }
    } else {
        quote! {
            let mut last_field_num = 0;
            #(#encode_fields)*
            
            // End of struct marked by delta 0
            encoder.write_uint(0)?;
            Ok(())
        }
    };
    
    // Update generated impl:
    // ...
    // pub fn encode(...) { #encode_impl }
    // ...
    
    // For now, let's assume Default is implemented for the struct.
    
    let struct_name = &item.ident;
    let type_id = gob_args.id.unwrap_or(0);
    
    // Check if we need to interpret as map
    let interpret_as_map = gob_args.interpret_as.as_ref().map_or(false, |s| s.starts_with("map["));
    
    let decode_impl = if interpret_as_map {
        // Map decoding logic
        // We need to map struct fields to map keys.
        // We will assume map keys are strings matching the field names (or `gob(name=...)` override).
        
        // let mut map_match_arms = Vec::new();
        
        if let Data::Struct(ref data) = item.data {
            if let Fields::Named(ref fields) = data.fields {
                for field in &fields.named {
                    let field_ident = field.ident.as_ref().unwrap();
                    let field_name_str = field_ident.to_string();
                    
                    // Recover custom name from attributes which we stripped earlier?
                    // Ah, we stripped them from `item` but we are iterating `item` here?
                    // Wait, `item` was modified in place above (stripping attributes).
                    // BUT we didn't save the custom names in a way easy to access here except by re-parsing or saving earlier.
                    // We should have saved the mapping earlier.
                    
                    // Let's rely on `field_ident` string for now, or we need to refactor the loop above to collect info.
                    // Refactoring loop above is better.
                }
            }
        }
        
        // Placeholder for the better implementation below
        quote! {
            // NOTE: We assume the decoder is positioned at the start of the Map value content
            // (after any headers).
            // A Gob Map on wire: [Count] [Key] [Value] [Key] [Value]...
            // `decoder.read_uint()` gives the count.
            
            // However, our generated code is called by `GobDecodable::decode` (conceptually),
            // which in turn is called by `Decoder`.
            // BUT `UserInfo::decode` is called manually in test.
            // If we call `UserInfo::decode(&mut decoder)`, it executes this block.
            
            // Debugging: print what we are doing
            // println!("Decoding UserInfo as map...");
            
            // The first thing in a map is the element count.
            let count = decoder.read_uint()?;
            // println!("Map count: {}", count);
            
            for _ in 0..count {
                let key_val = gobx::Value::decode(decoder)?;
                let value_val = gobx::Value::decode(decoder)?; 
                
                // println!("Key: {:?}, Value: {:?}", key_val, value_val);

                if let gobx::Value::String(key_str) = key_val {
                    match key_str.as_str() {
                        #(#map_decode_fields)*
                        _ => {
                            // Ignore unknown fields
                        }
                    }
                }
            }
            Ok(result)
        } 
    } else {
        // Standard struct delta decoding
        quote! {
                let mut field_num = -1i64;
                
                loop {
                    let delta = decoder.read_uint()?;
                    if delta == 0 { break; }
                    field_num += delta as i64;
                    
                    match field_num {
                        #(#decode_fields)*
                        _ => {
                            return Err(std::io::Error::new(std::io::ErrorKind::Other, format!("Unknown field delta {} (total {}) for struct {}", delta, field_num, stringify!(#struct_name))));
                        }
                    }
                }
                Ok(result)
        }
    };
    
    let expanded = quote! {
        #item

        impl gobx::GobType for #struct_name {
            const ID: i64 = #type_id;
        }
        
        impl gobx::GobDecodable for #struct_name {
            fn decode<R: std::io::Read>(decoder: &mut gobx::Decoder<R>) -> std::io::Result<Self> {
                 // We require Default for decode construction
                 Self::decode_struct(decoder)
            }
        }
        
        impl #struct_name {
            pub fn encode<W: std::io::Write>(&self, encoder: &mut gobx::Encoder<W>) -> std::io::Result<()> {
                #encode_impl
            }
            
            pub fn decode<R: std::io::Read>(decoder: &mut gobx::Decoder<R>) -> std::io::Result<Self> 
            where Self: Default {
                Self::decode_struct(decoder)
            }

            pub fn decode_struct<R: std::io::Read>(decoder: &mut gobx::Decoder<R>) -> std::io::Result<Self> 
            where Self: Default {
                let mut result = Self::default();
                #decode_impl
            }
        }
    };

    TokenStream::from(expanded)
}

