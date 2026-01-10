use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Data, Fields, Token};

fn camel_case(s: &str) -> String {
    stringcase::camel_case(s)
}

#[proc_macro_derive(Json, attributes(json))]
pub fn derive_json(token_stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_json2(syn::parse2::<syn::DeriveInput>(token_stream.into()).expect("DeriveInput")).into()
}

#[derive(Debug,PartialEq,Eq)]
enum JsonParam {
    Unit(String), NameValue(String, String)
}

impl Parse for JsonParam {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<syn::Ident>()?;
        Ok(if input.lookahead1().peek(Token![=]) {
            input.parse::<Token![=]>()?;
            JsonParam::NameValue(ident.to_string(), input.parse::<syn::LitStr>()?.value())
        } else {
            JsonParam::Unit(ident.to_string())
        })
    }
}

fn filter_attrs(attrs: &[Attribute]) -> Vec<JsonParam> {
    attrs.iter().filter_map(|attr| {
        if let Some(ident) = attr.path().get_ident() {
            if ident.eq("json") {
                return Some(attr.parse_args::<JsonParam>().unwrap());
            }
        }
        None
    }).collect()
}

fn derive_json2(input: syn::DeriveInput) -> TokenStream {
    let ty = input.ident;
    let use_default = true;
    match input.data {
        Data::Struct(data_struct) => {
            match data_struct.fields {
                Fields::Named(named_fields) => {
                    let mut to_token_stream = TokenStream::new();
                    let mut from_token_stream = TokenStream::new();
                    for field in &named_fields.named {
                        let params = filter_attrs(&field.attrs);
                        let skip = params.iter().find(|&v| match v {
                            JsonParam::Unit(_) => false,
                            JsonParam::NameValue(s, _) => s.eq("skip"),
                        });
                        let field_ident = field.ident.as_ref().unwrap();
                        let field_name = camel_case(&field_ident.to_string());
                        let field_type = &field.ty;
                        if let Some(JsonParam::NameValue(_, skip_path)) = skip {
                            let path = syn::parse_str::<syn::Path>(&skip_path).unwrap();
                            to_token_stream.extend(quote! {
                                if !#path(&self.#field_ident) {
                                    map.insert(#field_name.to_string(), <#field_type as ::resoxide_json::Json>::to_token(&self.#field_ident)?);
                                }
                            });
                        } else {
                            to_token_stream.extend(quote! {
                                map.insert(#field_name.to_string(), <#field_type as ::resoxide_json::Json>::to_token(&self.#field_ident)?);
                            });
                        }
                        if use_default {
                            from_token_stream.extend(quote! {
                                #field_ident:
                                    map.get(#field_name)
                                        .map(<#field_type as ::resoxide_json::Json>::from_token)
                                        .transpose()?
                                        .unwrap_or_default(),
                            });
                        }
                    }
                    quote! {
                        impl ::resoxide_json::Json for #ty {
                            type Error = ::resoxide_json::Error;

                            fn to_token(&self) -> Result<::resoxide_json::Token, Self::Error> {
                                let mut map: ::resoxide_json::Map =
                                    ::resoxide_json::Map::default();

                                #to_token_stream

                                Ok(::resoxide_json::Token::Object(map))
                            }

                            fn from_token(token: &::resoxide_json::Token) -> Result<Self, Self::Error> {
                                match token {
                                    ::resoxide_json::Token::Object(map) => {
                                        Ok(Self { #from_token_stream })
                                    },
                                    _ => Err(::resoxide_json::Error),
                                }
                            }

                            fn error() -> Self::Error {
                                ::resoxide_json::Error
                            }
                        }
                    }
                }
                Fields::Unnamed(unnamed_fields) => {
                    if unnamed_fields.unnamed.len() == 1 {
                        let inner_ty = &unnamed_fields.unnamed[0].ty;
                        quote! {
                            impl ::resoxide_json::Json for #ty {
                                type Error = ::resoxide_json::Error;

                                fn to_token(&self) -> Result<::resoxide_json::Token, Self::Error> {
                                    <#inner_ty as ::resoxide_json::Json>::to_token(&self.0)
                                }

                                fn from_token(token: &::resoxide_json::Token) -> Result<Self, Self::Error> {
                                    Ok(#ty(<#inner_ty as ::resoxide_json::Json>::from_token(token)?))
                                }
                            }
                        }
                    } else {
                        panic!("Tuple structs are not supported");
                    }
                }
                Fields::Unit => {
                    todo!()
                }
            }
        }
        Data::Enum(data_enum) => {
            let mut from_token_stream = TokenStream::new();
            let mut to_token_stream = TokenStream::new();
            for variant in &data_enum.variants {
                let variant_ident = &variant.ident;
                let mut variant_name = camel_case(&variant_ident.to_string());
                for param in filter_attrs(&variant.attrs) {
                    match param {
                        JsonParam::NameValue(name, value) if name.eq("rename") => {
                            variant_name = value;
                        }
                        _ => ()
                    }
                }
                match &variant.fields {
                    Fields::Named(_) => {
                        todo!()
                    }
                    Fields::Unnamed(unnamed_fields) => {
                        if unnamed_fields.unnamed.len() == 1 {
                            let inner_ty = &unnamed_fields.unnamed[0].ty;
                            from_token_stream.extend(quote! {
                                #ty::#variant_ident(__value) => (#variant_name, <#inner_ty as ::resoxide_json::Json>::to_token(__value)?),
                            });
                            to_token_stream.extend(quote! {
                                #variant_name => Ok(#ty::#variant_ident(<#inner_ty as ::resoxide_json::Json>::from_token(token)?)),
                            });
                        } else {
                            panic!("Tuple structs are not supported");
                        }
                    }
                    Fields::Unit => {
                        from_token_stream.extend(quote! {
                            #ty::#variant_ident => (#variant_name, ::resoxide_json::Token::Object(::resoxide_json::Map::default())),
                        });
                        to_token_stream.extend(quote! {
                            #variant_name => Ok(#ty::#variant_ident),
                        });
                    }
                }
            }
            quote! {
                impl ::resoxide_json::Json for #ty {
                    type Error = ::resoxide_json::Error;

                    fn to_token(&self) -> Result<::resoxide_json::Token, Self::Error> {
                        let (ty, mut map) = match self {
                            #from_token_stream
                        };
                        match &mut map {
                            ::resoxide_json::Token::Object(m) => {
                                m.insert("$type".to_string(),
                                    <String as ::resoxide_json::Json>::to_token(&ty.to_string())?);
                            },
                            _ => return Err(::resoxide_json::Error),
                        };
                        Ok(map)
                    }

                    fn from_token(token: &::resoxide_json::Token) -> Result<Self, Self::Error> {
                        match token {
                            ::resoxide_json::Token::Object(map) => {
                                let variant = <String as ::resoxide_json::Json>::from_token(
                                    map.get("$type").ok_or(::resoxide_json::Error)?
                                    )?;

                                match variant.as_str() {
                                    #to_token_stream
                                    _ => Err(::resoxide_json::Error),
                                }
                            }
                            _ => Err(::resoxide_json::Error)
                        }
                    }

                    fn error() -> Self::Error {
                        ::resoxide_json::Error
                    }
                }

            }
        }
        Data::Union(_) => {
            panic!("Unions are not supported");
        }
    }
}