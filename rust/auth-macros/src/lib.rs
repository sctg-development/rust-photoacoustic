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

    // Parse arguments: path and permission, with optional route attributes
    let (path, permission, route_attrs) = match parse_protect_args_extended(&args) {
        Ok((p, perm, attrs)) => (p, perm, attrs),
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

    // Generate the appropriate Rocket attribute based on HTTP method with optional route attributes
    let rocket_attr = match http_method {
        "get" => {
            if route_attrs.is_empty() {
                quote! { #[rocket::get(#path)] }
            } else {
                quote! { #[rocket::get(#path, #route_attrs)] }
            }
        }
        "post" => {
            if route_attrs.is_empty() {
                quote! { #[rocket::post(#path)] }
            } else {
                quote! { #[rocket::post(#path, #route_attrs)] }
            }
        }
        "put" => {
            if route_attrs.is_empty() {
                quote! { #[rocket::put(#path)] }
            } else {
                quote! { #[rocket::put(#path, #route_attrs)] }
            }
        }
        "delete" => {
            if route_attrs.is_empty() {
                quote! { #[rocket::delete(#path)] }
            } else {
                quote! { #[rocket::delete(#path, #route_attrs)] }
            }
        }
        "patch" => {
            if route_attrs.is_empty() {
                quote! { #[rocket::patch(#path)] }
            } else {
                quote! { #[rocket::patch(#path, #route_attrs)] }
            }
        }
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
        // Add OAuthBearer parameter as the first parameter (before route parameters)
        // This ensures it's treated as a FromRequest guard, not interfering with route parsing
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
///
/// // With route attributes (rank, format, data, etc.)
/// #[protect_get("/path", "permission:scope", rank = 1, format = "json")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
/// }
/// ```
///
/// ### Supported Route Grammar
///
/// The macro supports the full Rocket route grammar including:
/// - Path parameters: `/users/<id>` (requires `FromParam`)
/// - Trailing segments: `/files/<path..>` (requires `FromSegments`)
/// - Query parameters: `/search?<query>` (requires `FromForm`)
/// - Data parameters: `data = "<payload>"` (requires `FromData`)
/// - Route ranking: `rank = 1`
/// - Content type matching: `format = "json"`
///
/// ### Examples
///
/// ```rust,ignore
/// // Simple protected route
/// #[protect_get("/api/data", "read:api")]
/// fn get_data() -> Json<&'static str> {
///     Json("Protected data")
/// }
///
/// // With path parameters
/// #[protect_get("/users/<id>", "read:users")]
/// fn get_user(id: u32) -> Json<User> {
///     // id parameter automatically parsed from path
///     Json(User::find(id))
/// }
///
/// // With query parameters
/// #[protect_get("/search?<query>", "read:search")]
/// fn search(query: SearchForm) -> Json<Vec<Result>> {
///     // query automatically parsed from query string
///     Json(search_service(query))
/// }
///
/// // With route attributes
/// #[protect_get("/api/data", "read:api", rank = 2, format = "json")]
/// fn get_data_json() -> Json<Data> {
///     Json(get_data())
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
#[allow(dead_code)]
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

/// Parse the arguments for protection macros with extended route attributes support
/// Supports: path, permission, and optional route attributes like rank, format, data
fn parse_protect_args_extended(
    args: &Punctuated<Expr, Token![,]>,
) -> Result<(String, String, proc_macro2::TokenStream), String> {
    if args.len() < 2 {
        return Err(
            "Protection macros require at least 2 arguments: path and permission".to_string(),
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

    // Collect remaining arguments as route attributes (rank, format, data, etc.)
    let route_attrs = if args.len() > 2 {
        let remaining_args: Vec<_> = args.iter().skip(2).collect();
        quote::quote! { #(#remaining_args),* }
    } else {
        proc_macro2::TokenStream::new()
    };

    Ok((path, permission, route_attrs))
}

/// Parse the arguments for OpenAPI protection macros with optional tag and route attributes
fn parse_openapi_protect_args(
    args: &Punctuated<Expr, Token![,]>,
) -> Result<(String, String, Option<String>, proc_macro2::TokenStream), String> {
    if args.len() < 2 {
        return Err(
            format!("OpenAPI protection macros require at least 2 arguments: path and permission. Got {} arguments.", args.len())
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

    // Look for tag assignment and collect other route attributes
    let mut tag = None;
    let mut route_attrs = Vec::new();

    for arg in args.iter().skip(2) {
        match arg {
            Expr::Assign(assign) => {
                // Check if left side is specifically "tag"
                if let Expr::Path(path) = &*assign.left {
                    if path.path.segments.len() == 1 && path.path.segments[0].ident == "tag" {
                        // This is a tag assignment - check if right side is a string literal
                        if let Expr::Lit(expr_lit) = &*assign.right {
                            if let Lit::Str(lit_str) = &expr_lit.lit {
                                tag = Some(lit_str.value());
                                continue; // Don't add to route_attrs
                            } else {
                                return Err("tag value must be a string literal".to_string());
                            }
                        } else {
                            return Err("tag value must be a string literal".to_string());
                        }
                    }
                }
                // Not a tag assignment, treat as route attribute
                route_attrs.push(arg);
            }
            _ => {
                // Any other expression is treated as a route attribute
                route_attrs.push(arg);
            }
        }
    }

    let route_attrs_tokens = if !route_attrs.is_empty() {
        quote::quote! { #(#route_attrs),* }
    } else {
        proc_macro2::TokenStream::new()
    };

    Ok((path, permission, tag, route_attrs_tokens))
}

/// Internal function that implements the combined OpenAPI + protection logic for all HTTP methods
fn openapi_protect_universal_impl(
    args: TokenStream,
    input: TokenStream,
    http_method: &str,
) -> TokenStream {
    let args = parse_macro_input!(args with Punctuated::<Expr, Token![,]>::parse_terminated);
    let input_fn = parse_macro_input!(input as ItemFn);

    // Parse arguments: path, permission, optional tag, and route attributes
    let (path, permission, tag, route_attrs) = match parse_openapi_protect_args(&args) {
        Ok((p, perm, t, attrs)) => (p, perm, t, attrs),
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
        "get" => {
            if route_attrs.is_empty() {
                quote! { #[get(#path)] }
            } else {
                quote! { #[get(#path, #route_attrs)] }
            }
        }
        "post" => {
            if route_attrs.is_empty() {
                quote! { #[post(#path)] }
            } else {
                quote! { #[post(#path, #route_attrs)] }
            }
        }
        "put" => {
            if route_attrs.is_empty() {
                quote! { #[put(#path)] }
            } else {
                quote! { #[put(#path, #route_attrs)] }
            }
        }
        "delete" => {
            if route_attrs.is_empty() {
                quote! { #[delete(#path)] }
            } else {
                quote! { #[delete(#path, #route_attrs)] }
            }
        }
        "patch" => {
            if route_attrs.is_empty() {
                quote! { #[patch(#path)] }
            } else {
                quote! { #[patch(#path, #route_attrs)] }
            }
        }
        _ => {
            return syn::Error::new_spanned(
                &input_fn,
                format!("Unsupported HTTP method: {}", http_method),
            )
            .to_compile_error()
            .into()
        }
    };

    // Generate the OpenAPI attribute with optional tag
    let openapi_attr = if let Some(tag_value) = tag {
        quote! { #[rocket_okapi::openapi(tag = #tag_value)] }
    } else {
        quote! { #[rocket_okapi::openapi] }
    };

    // Generate the combined function with OpenAPI + Either return type
    let expanded = if has_bearer_param {
        // If OAuthBearer is already in signature, just add permission check
        quote! {
            // Actual function implementation with OpenAPI attribute BEFORE route attribute
            #(#fn_attrs)*
            #openapi_attr
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
        // Add OAuthBearer parameter as the first parameter (before route parameters)
        // This ensures it's treated as a FromRequest guard, not interfering with route parsing
        quote! {
            // Actual function implementation with OpenAPI attribute BEFORE route attribute
            #(#fn_attrs)*
            #openapi_attr
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
///
/// // With optional tag for OpenAPI documentation
/// #[openapi_protect_get("/path", "permission:scope", tag = "Custom Tag")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available  
/// }
///
/// // With route attributes and tag
/// #[openapi_protect_get("/users/<id>", "read:users", tag = "Users", rank = 1)]
/// fn get_user(id: u32) -> Json<User> {
///     // Full route grammar support
/// }
/// ```
///
/// ### Parameters
///
/// - `path`: The route path (required) - supports full Rocket route grammar
/// - `permission`: The required permission string (required)
/// - `tag`: Optional OpenAPI tag for grouping endpoints in documentation
/// - Additional route attributes: `rank`, `format`, `data`, etc.
///
/// ### Supported Route Grammar
///
/// The macro supports the full Rocket route grammar including:
/// - Path parameters: `/users/<id>` (requires `FromParam`)
/// - Trailing segments: `/files/<path..>` (requires `FromSegments`)
/// - Query parameters: `/search?<query>` (requires `FromForm`)
/// - Data parameters: `data = "<payload>"` (requires `FromData`)
/// - Route ranking: `rank = 1`
/// - Content type matching: `format = "json"`
///
/// ### Features
///
/// - Automatically adds `OAuthBearer` request guard to function signature
/// - Generates proper OpenAPI documentation including authentication requirements
/// - Supports optional tag parameter for OpenAPI documentation organization
/// - Supports all Rocket route attributes for advanced routing
/// - Adds permission checking logic
/// - Returns HTTP 403 Forbidden if permission is denied
/// - Uses `rocket::Either` for proper response type handling
///
/// ### Examples
///
/// ```rust,ignore
/// use auth_macros::openapi_protect_get;
/// use rocket::serde::json::Json;
///
/// #[openapi_protect_get("/api/data", "read:api")]
/// fn get_data() -> Json<&'static str> {
///     Json("Protected data")
/// }
///
/// #[openapi_protect_get("/users/<id>", "read:users", tag = "User Management")]
/// fn get_user(id: u32) -> Json<User> {
///     Json(User::find(id))
/// }
///
/// #[openapi_protect_post("/users", "create:users", tag = "Users", data = "<user>")]
/// fn create_user(user: Json<NewUser>) -> Json<User> {
///     Json(User::create(user.into_inner()))
/// }
///
/// #[openapi_protect_get("/search?<query>", "read:search", rank = 2)]
/// fn search(query: SearchForm) -> Json<Vec<Result>> {
///     Json(search_service(query))
/// }
/// ```
#[proc_macro_attribute]
pub fn openapi_protect_get(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "get")
}

/// Combined OpenAPI and protection macro for POST routes
///
/// This macro combines the functionality of `#[openapi]` and `#[protect_post]` into a single
/// attribute that automatically generates OpenAPI documentation and adds Bearer token validation
/// with permission checking.
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_post("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
///
/// // With optional tag for OpenAPI documentation
/// #[openapi_protect_post("/path", "permission:scope", tag="Custom Tag")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available  
/// }
/// ```
///
/// ### Parameters
///
/// - `path`: The route path (required)
/// - `permission`: The required permission string (required)
/// - `tag`: Optional OpenAPI tag for grouping endpoints in documentation
///
/// ### Features
///
/// - Automatically adds `OAuthBearer` request guard to function signature
/// - Generates proper OpenAPI documentation including authentication requirements
/// - Supports optional tag parameter for OpenAPI documentation organization
/// - Adds permission checking logic
/// - Returns HTTP 403 Forbidden if permission is denied
/// - Uses `rocket::Either` for proper response type handling
#[proc_macro_attribute]
pub fn openapi_protect_post(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "post")
}

/// Combined OpenAPI and protection macro for PUT routes
///
/// This macro combines the functionality of `#[openapi]` and `#[protect_put]` into a single
/// attribute that automatically generates OpenAPI documentation and adds Bearer token validation
/// with permission checking.
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_put("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
///
/// // With optional tag for OpenAPI documentation
/// #[openapi_protect_put("/path", "permission:scope", tag="Custom Tag")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available  
/// }
/// ```
///
/// ### Parameters
///
/// - `path`: The route path (required)
/// - `permission`: The required permission string (required)
/// - `tag`: Optional OpenAPI tag for grouping endpoints in documentation
///
/// ### Features
///
/// - Automatically adds `OAuthBearer` request guard to function signature
/// - Generates proper OpenAPI documentation including authentication requirements
/// - Supports optional tag parameter for OpenAPI documentation organization
/// - Adds permission checking logic
/// - Returns HTTP 403 Forbidden if permission is denied
/// - Uses `rocket::Either` for proper response type handling
#[proc_macro_attribute]
pub fn openapi_protect_put(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "put")
}

/// Combined OpenAPI and protection macro for DELETE routes
///
/// This macro combines the functionality of `#[openapi]` and `#[protect_delete]` into a single
/// attribute that automatically generates OpenAPI documentation and adds Bearer token validation
/// with permission checking.
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_delete("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
///
/// // With optional tag for OpenAPI documentation
/// #[openapi_protect_delete("/path", "permission:scope", tag="Custom Tag")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available  
/// }
/// ```
///
/// ### Parameters
///
/// - `path`: The route path (required)
/// - `permission`: The required permission string (required)
/// - `tag`: Optional OpenAPI tag for grouping endpoints in documentation
///
/// ### Features
///
/// - Automatically adds `OAuthBearer` request guard to function signature
/// - Generates proper OpenAPI documentation including authentication requirements
/// - Supports optional tag parameter for OpenAPI documentation organization
/// - Adds permission checking logic
/// - Returns HTTP 403 Forbidden if permission is denied
/// - Uses `rocket::Either` for proper response type handling
#[proc_macro_attribute]
pub fn openapi_protect_delete(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "delete")
}

/// Combined OpenAPI and protection macro for PATCH routes
///
/// This macro combines the functionality of `#[openapi]` and `#[protect_patch]` into a single
/// attribute that automatically generates OpenAPI documentation and adds Bearer token validation
/// with permission checking.
///
/// ### Syntax
///
/// ```rust,ignore
/// #[openapi_protect_patch("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
///
/// // With optional tag for OpenAPI documentation
/// #[openapi_protect_patch("/path", "permission:scope", tag="Custom Tag")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available  
/// }
/// ```
///
/// ### Parameters
///
/// - `path`: The route path (required)
/// - `permission`: The required permission string (required)
/// - `tag`: Optional OpenAPI tag for grouping endpoints in documentation
///
/// ### Features
///
/// - Automatically adds `OAuthBearer` request guard to function signature
/// - Generates proper OpenAPI documentation including authentication requirements
/// - Supports optional tag parameter for OpenAPI documentation organization
/// - Adds permission checking logic
/// - Returns HTTP 403 Forbidden if permission is denied
/// - Uses `rocket::Either` for proper response type handling
#[proc_macro_attribute]
pub fn openapi_protect_patch(args: TokenStream, input: TokenStream) -> TokenStream {
    openapi_protect_universal_impl(args, input, "patch")
}
