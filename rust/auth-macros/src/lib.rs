// Copyright (c) 2025 Ronan LE MEILLAT, SCTG Development
// This file is part of the rust-photoacoustic project and is licensed under the
// SCTG Development Non-Commercial License v1.0 (see LICENSE.md for details).

//! Procedural macros for creating protected routes with automatic permission checking

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, Expr, ItemFn, Lit, Token};

/// Attribute macro for creating protected GET routes with permission checking
///
/// This macro automatically adds Bearer token validation and permission checking
/// to Rocket route handlers. If the user doesn't have the required permission,
/// it returns HTTP 403 Forbidden. The macro uses `rocket::Either` to handle
/// both success and error responses properly.
///
/// # How it works
///
/// 1. **Automatic Bearer Token Injection**: If `OAuthBearer` is not in the function signature,
///    the macro automatically adds it as the first parameter
/// 2. **Permission Checking**: Validates that the authenticated user has the required permission
/// 3. **Type-Safe Returns**: Uses `rocket::Either<Forbidden, T>` to return either a 403 error
///    or the original function's return type
/// 4. **Bearer Token Access**: The `bearer` variable is available in the function scope
///
/// # Syntax
///
/// ```rust,ignore
/// #[protect_get("/path", "permission:scope")]
/// fn handler_name() -> SomeResponse {
///     // Your handler code here
///     // The 'bearer' variable is automatically available
/// }
/// ```
///
/// # Examples
///
/// ## Simple protected route (automatic Bearer injection)
/// ```rust,ignore
/// #[protect_get("/admin/users", "admin:users")]
/// fn list_users() -> Json<Vec<User>> {
///     // The macro automatically injects: bearer: OAuthBearer
///     // and checks for "admin:users" permission
///     Json(vec![
///         User {
///             id: bearer.user_info.user_id.clone(),
///             name: "Current User".to_string(),
///         }
///     ])
/// }
/// ```
///
/// ## Explicit Bearer parameter
/// ```rust,ignore
/// #[protect_get("/api/data", "read:data")]
/// fn get_data(bearer: OAuthBearer) -> Json<DataResponse> {
///     // When Bearer is explicitly in signature, macro just adds permission checking
///     Json(DataResponse {
///         user_id: bearer.user_info.user_id,
///         data: fetch_user_data(&bearer.user_info.user_id),
///     })
/// }
/// ```
///
/// ## With additional route parameters
/// ```rust,ignore
/// #[protect_get("/api/user/<user_id>/profile", "read:profiles")]
/// fn get_user_profile(user_id: String) -> Json<Profile> {
///     // Route parameters work normally, bearer is auto-injected
///     Json(Profile {
///         user_id: user_id,
///         viewer: bearer.user_info.user_id.clone(),
///     })
/// }
/// ```
///
/// # Return Type
///
/// The macro transforms the function to return:
/// ```rust,ignore
/// rocket::Either<rocket::response::status::Forbidden<&'static str>, OriginalReturnType>
/// ```
///
/// - **Left**: 403 Forbidden with "Permission denied" message if permission check fails
/// - **Right**: Original function's return value if permission check passes
///
/// # HTTP Responses
///
/// | Condition | Response | Description |
/// |-----------|----------|-------------|
/// | Missing/invalid token | 401 Unauthorized | Handled by `OAuthBearer` guard |
/// | Valid token, insufficient permissions | 403 Forbidden | Returned by macro |
/// | Valid token, sufficient permissions | Original response | Function executes normally |
#[proc_macro_attribute]
pub fn protect_get(args: TokenStream, input: TokenStream) -> TokenStream {
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

    // Generate the protected function with Either return type
    let expanded = if has_bearer_param {
        // If OAuthBearer is already in signature, just add permission check
        quote! {
            #(#fn_attrs)*
            #[rocket::get(#path)]
            #fn_vis fn #fn_name(#fn_inputs) -> rocket::Either<rocket::response::status::Forbidden<&'static str>, #return_type> {
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
            #[rocket::get(#path)]
            #fn_vis fn #fn_name(
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

/// Parse the arguments for protect_get macro
fn parse_protect_args(args: &Punctuated<Expr, Token![,]>) -> Result<(String, String), String> {
    if args.len() != 2 {
        return Err("protect_get requires exactly 2 arguments: path and permission".to_string());
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
