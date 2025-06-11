// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Procedural macros for creating protected routes with automatic permission checking

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, Expr, ItemFn, Lit, Token};

/// Internal function that implements the protection logic for all HTTP methods
fn protect_universal_impl(args: TokenStream, input: TokenStream, http_method: &str) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Expr, Token![,]>::parse_terminated);
    let input_fn = parse_macro_input!(input as ItemFn);

    // Parse arguments: path and permission
    let (path, permission) = match parse_protect_args(&args) {
        Ok((p, perm)) => (p, perm),
        Err(err) => {
            return syn::Error::new_spanned(&input_fn, err)
                .to_compile_error()
                .into()
        }
    };

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_attrs = &input_fn.attrs;
    let fn_asyncness = &input_fn.sig.asyncness;

    // Extract the return type from the function signature
    let return_type = match fn_output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    // Check if OAuthBearer is already in the function signature
    let has_bearer_param = fn_inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Type::Path(type_path) = &*pat_type.ty {
                return type_path
                    .path
                    .segments
                    .last()
                    .map(|seg| seg.ident == "OAuthBearer")
                    .unwrap_or(false);
            }
        }
        false
    });

    // Generate the appropriate Rocket attribute based on HTTP method
    let rocket_attr = match http_method {
        "get" => quote! { #[rocket::get(#path)] },
        "post" => quote! { #[rocket::post(#path)] },
        "put" => quote! { #[rocket::put(#path)] },
        "delete" => quote! { #[rocket::delete(#path)] },
        "patch" => quote! { #[rocket::patch(#path)] },
        _ => {
            return syn::Error::new_spanned(
                &input_fn,
                format!("Unsupported HTTP method: {}", http_method),
            )
            .to_compile_error()
            .into()
        }
    };

    // Generate the protected function with Either return type
    let expanded = if has_bearer_param {
        // If OAuthBearer is already in signature, just add permission check
        quote! {
            #(#fn_attrs)*
            #rocket_attr
            #fn_vis #fn_asyncness fn #fn_name(#fn_inputs) -> rocket::Either<rocket::response::status::Forbidden<&'static str>, #return_type> {
                // Check permission first
                if !bearer.has_permission(#permission) {
                    return rocket::Either::Left(rocket::response::status::Forbidden("Permission denied"));
                }

                // Call original function and wrap in Either::Right
                rocket::Either::Right(#fn_block)
            }
        }
    } else {
        // Add OAuthBearer parameter and permission check
        quote! {
            #(#fn_attrs)*
            #rocket_attr
            #fn_vis #fn_asyncness fn #fn_name(
                bearer: crate::visualization::auth::guards::OAuthBearer,
                #fn_inputs
            ) -> rocket::Either<rocket::response::status::Forbidden<&'static str>, #return_type> {
                // Check permission
                if !bearer.has_permission(#permission) {
                    return rocket::Either::Left(rocket::response::status::Forbidden("Permission denied"));
                }

                // Call original function and wrap in Either::Right - the bearer variable is now available in scope
                rocket::Either::Right(#fn_block)
            }
        }
    };

    expanded.into()
}

/// Attribute macro for creating protected GET routes with permission checking
///
/// This macro automatically adds Bearer token validation and permission checking
/// to Rocket route handlers. If the user doesn't have the required permission,
/// it returns HTTP 403 Forbidden. The macro uses `rocket::Either` to handle
/// both success and error responses properly.
///
/// ### Syntax
///
/// ```rust,ignore
/// #[protect_get("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn protect_get(args: TokenStream, input: TokenStream) -> TokenStream {
    protect_universal_impl(args, input, "get")
}

/// Attribute macro for creating protected POST routes with permission checking
///
/// ### Syntax
///
/// ```rust,ignore
/// #[protect_post("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn protect_post(args: TokenStream, input: TokenStream) -> TokenStream {
    protect_universal_impl(args, input, "post")
}

/// Attribute macro for creating protected PUT routes with permission checking
///
/// ### Syntax
///
/// ```rust,ignore
/// #[protect_put("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn protect_put(args: TokenStream, input: TokenStream) -> TokenStream {
    protect_universal_impl(args, input, "put")
}

/// Attribute macro for creating protected DELETE routes with permission checking
///
/// ### Syntax
///
/// ```rust,ignore
/// #[protect_delete("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn protect_delete(args: TokenStream, input: TokenStream) -> TokenStream {
    protect_universal_impl(args, input, "delete")
}

/// Attribute macro for creating protected PATCH routes with permission checking
///
/// ### Syntax
///
/// ```rust,ignore
/// #[protect_patch("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
#[proc_macro_attribute]
pub fn protect_patch(args: TokenStream, input: TokenStream) -> TokenStream {
    protect_universal_impl(args, input, "patch")
}

/// Parse the arguments for protection macros
fn parse_protect_args(args: &Punctuated<Expr, Token![,]>) -> Result<(String, String), String> {
    if args.len() != 2 {
        return Err(
            "Protection macros require exactly 2 arguments: path and permission".to_string(),
        );
    }

    let path = match &args[0] {
        Expr::Lit(expr_lit) => {
            if let Lit::Str(lit_str) = &expr_lit.lit {
                lit_str.value()
            } else {
                return Err("First argument (path) must be a string literal".to_string());
            }
        }
        _ => return Err("First argument (path) must be a string literal".to_string()),
    };

    let permission = match &args[1] {
        Expr::Lit(expr_lit) => {
            if let Lit::Str(lit_str) = &expr_lit.lit {
                lit_str.value()
            } else {
                return Err("Second argument (permission) must be a string literal".to_string());
            }
        }
        _ => return Err("Second argument (permission) must be a string literal".to_string()),
    };

    Ok((path, permission))
}

/// Internal function that implements the combined OpenAPI + protection logic for all HTTP methods
fn openapi_protect_universal_impl(
    args: TokenStream,
    input: TokenStream,
    http_method: &str,
) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Expr, Token![,]>::parse_terminated);
    let input_fn = parse_macro_input!(input as ItemFn);

    // Parse arguments: path and permission
    let (path, permission) = match parse_protect_args(&args) {
        Ok((p, perm)) => (p, perm),
        Err(err) => {
            return syn::Error::new_spanned(&input_fn, err)
                .to_compile_error()
                .into()
        }
    };

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_inputs = &input_fn.sig.inputs;
    let fn_output = &input_fn.sig.output;
    let fn_attrs = &input_fn.attrs;
    let fn_asyncness = &input_fn.sig.asyncness;

    // Extract the return type from the function signature
    let return_type = match fn_output {
        syn::ReturnType::Default => quote! { () },
        syn::ReturnType::Type(_, ty) => quote! { #ty },
    };

    // Check if OAuthBearer is already in the function signature
    let has_bearer_param = fn_inputs.iter().any(|arg| {
        if let syn::FnArg::Typed(pat_type) = arg {
            if let syn::Type::Path(type_path) = &*pat_type.ty {
                return type_path
                    .path
                    .segments
                    .last()
                    .map(|seg| seg.ident == "OAuthBearer")
                    .unwrap_or(false);
            }
        }
        false
    });

    // Generate the appropriate Rocket attribute based on HTTP method (for OpenAPI combined macro)
    let rocket_attr = match http_method {
        "get" => quote! { #[get(#path)] },
        "post" => quote! { #[post(#path)] },
        "put" => quote! { #[put(#path)] },
        "delete" => quote! { #[delete(#path)] },
        "patch" => quote! { #[patch(#path)] },
        _ => {
            return syn::Error::new_spanned(
                &input_fn,
                format!("Unsupported HTTP method: {}", http_method),
            )
            .to_compile_error()
            .into()
        }
    };

    // Generate the combined function with OpenAPI + Either return type
    let expanded = if has_bearer_param {
        // If OAuthBearer is already in signature, just add permission check
        quote! {
            // Actual function implementation with OpenAPI attribute BEFORE route attribute
            #(#fn_attrs)*
            #[rocket_okapi::openapi]
            #rocket_attr
            #fn_vis #fn_asyncness fn #fn_name(#fn_inputs) -> rocket::Either<rocket::response::status::Forbidden<&'static str>, #return_type> {
                // Check permission first
                if !bearer.has_permission(#permission) {
                    return rocket::Either::Left(rocket::response::status::Forbidden("Permission denied"));
                }

                // Call original function and wrap in Either::Right
                rocket::Either::Right(#fn_block)
            }
        }
    } else {
        // Add OAuthBearer parameter and permission check
        quote! {
            // Actual function implementation with OpenAPI attribute BEFORE route attribute
            #(#fn_attrs)*
            #[rocket_okapi::openapi]
            #rocket_attr
            #fn_vis #fn_asyncness fn #fn_name(
                bearer: crate::visualization::auth::guards::OAuthBearer,
                #fn_inputs
            ) -> rocket::Either<rocket::response::status::Forbidden<&'static str>, #return_type> {
                // Check permission
                if !bearer.has_permission(#permission) {
                    return rocket::Either::Left(rocket::response::status::Forbidden("Permission denied"));
                }

                // Call original function and wrap in Either::Right - the bearer variable is now available in scope
                rocket::Either::Right(#fn_block)
            }
        }
    };

    expanded.into()
}

/// Combined OpenAPI and protection macro for GET routes
///
/// This macro combines the functionality of `#[openapi]` and `#[protect_get]` into a single
/// attribute that automatically generates OpenAPI documentation and adds Bearer token validation
/// with permission checking.
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_get("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
///
/// ### Features
///
/// - Automatically adds `OAuthBearer` request guard to function signature
/// - Generates proper OpenAPI documentation including authentication requirements
/// - Adds permission checking logic
/// - Returns HTTP 403 Forbidden if permission is denied
/// - Uses `rocket::Either` for proper response type handling
///
/// ### Example
///
/// ```rust,ignore
/// use auth_macros::openapi_protect_get;
/// use rocket::serde::json::Json;
///
/// #[openapi_protect_get("/api/data", "read:api")]
/// fn get_data() -> Json<&'static str> {
///     Json("Protected data")
/// }
/// ```
#[proc_macro_attribute]
pub fn openapi_protect_get(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "get")
}

/// Combined OpenAPI and protection macro for POST routes
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_post("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn openapi_protect_post(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "post")
}

/// Combined OpenAPI and protection macro for PUT routes
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_put("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn openapi_protect_put(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "put")
}

/// Combined OpenAPI and protection macro for DELETE routes
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_delete("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn openapi_protect_delete(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "delete")
}

/// Combined OpenAPI and protection macro for PATCH routes
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_patch("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
#[proc_macro_attribute]
pub fn openapi_protect_patch(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "patch")
}
