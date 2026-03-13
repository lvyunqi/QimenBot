use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Ident, ItemMod, LitStr, Token,
};

// ─── Plugin-level args ──────────────────────────────────────────────────

// Parse: id = "...", version = "..."
struct PluginArgs {
    id: String,
    version: String,
}

impl Parse for PluginArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut id = None;
        let mut version = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "id" => id = Some(value.value()),
                "version" => version = Some(value.value()),
                other => {
                    return Err(syn::Error::new(key.span(), format!("unknown key: {other}")))
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(PluginArgs {
            id: id.ok_or_else(|| input.error("missing `id`"))?,
            version: version.ok_or_else(|| input.error("missing `version`"))?,
        })
    }
}

// ─── #[command(...)] args ───────────────────────────────────────────────

// Parse: name = "...", description = "...", aliases = "...", category = "...", role = "..."
struct CommandArgs {
    name: String,
    description: String,
    aliases: String,
    category: String,
    role: String,
    scope: String,
}

impl Parse for CommandArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut aliases = String::new();
        let mut category = String::new();
        let mut role = String::new();
        let mut scope = String::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "name" => name = Some(value.value()),
                "description" => description = Some(value.value()),
                "aliases" => aliases = value.value(),
                "category" => category = value.value(),
                "role" => role = value.value(),
                "scope" => scope = value.value(),
                other => {
                    return Err(syn::Error::new(key.span(), format!("unknown key: {other}")))
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(CommandArgs {
            name: name.ok_or_else(|| input.error("missing `name`"))?,
            description: description.ok_or_else(|| input.error("missing `description`"))?,
            aliases,
            category,
            role,
            scope,
        })
    }
}

// ─── #[route(...)] args ─────────────────────────────────────────────────

// Parse: kind = "notice", events = "GroupPoke,PrivatePoke"
struct RouteArgs {
    kind: String,
    events: String,
}

impl Parse for RouteArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut kind = None;
        let mut events = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "kind" => kind = Some(value.value()),
                "events" => events = Some(value.value()),
                other => {
                    return Err(syn::Error::new(key.span(), format!("unknown key: {other}")))
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(RouteArgs {
            kind: kind.ok_or_else(|| input.error("missing `kind`"))?,
            events: events.ok_or_else(|| input.error("missing `events`"))?,
        })
    }
}

// ─── Macro entry point ──────────────────────────────────────────────────

/// Attribute macro for declaring a dynamic plugin module.
///
/// Usage:
/// ```ignore
/// #[dynamic_plugin(id = "my-plugin", version = "0.1.0")]
/// mod my_plugin {
///     #[command(name = "greet", description = "Say hello", aliases = "hi,hello")]
///     fn greet(req: &CommandRequest) -> CommandResponse {
///         CommandResponse::text("Hello!")
///     }
///
///     #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
///     fn on_poke(req: &NoticeRequest) -> NoticeResponse { ... }
///
///     #[init]
///     fn my_init(config: PluginInitConfig) -> PluginInitResult { ... }
///
///     #[shutdown]
///     fn my_shutdown() { ... }
/// }
/// ```
#[proc_macro_attribute]
pub fn dynamic_plugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let plugin_args = parse_macro_input!(attr as PluginArgs);
    let module = parse_macro_input!(item as ItemMod);

    match expand_dynamic_plugin(plugin_args, module) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_dynamic_plugin(args: PluginArgs, mut module: ItemMod) -> syn::Result<TokenStream2> {
    let mod_name = &module.ident;
    let mod_vis = &module.vis;
    let mod_attrs = &module.attrs;

    let Some((_brace, ref mut items)) = module.content else {
        return Err(syn::Error::new_spanned(
            &module,
            "module must have inline content (not `mod foo;`)",
        ));
    };

    let mut command_entries = Vec::new();
    let mut route_entries = Vec::new();
    let mut init_fn: Option<String> = None;
    let mut shutdown_fn: Option<String> = None;
    let mut pre_handle_fn: Option<String> = None;
    let mut after_completion_fn: Option<String> = None;
    let mut transformed_items = Vec::new();

    for item in items.drain(..) {
        match item {
            syn::Item::Fn(mut func) => {
                // Check for #[command(...)]
                if let Some((cmd_tokens, remaining_attrs)) = extract_attr(&func.attrs, "command")? {
                    let cmd_args: CommandArgs = syn::parse2(cmd_tokens)?;
                    func.attrs = remaining_attrs;
                    let fn_name = func.sig.ident.to_string();

                    make_extern_c(&mut func);
                    command_entries.push((fn_name, cmd_args));
                    transformed_items.push(syn::Item::Fn(func));
                }
                // Check for #[route(...)]
                else if let Some((route_tokens, remaining_attrs)) =
                    extract_attr(&func.attrs, "route")?
                {
                    let route_args: RouteArgs = syn::parse2(route_tokens)?;
                    func.attrs = remaining_attrs;
                    let fn_name = func.sig.ident.to_string();

                    make_extern_c(&mut func);
                    route_entries.push((fn_name, route_args));
                    transformed_items.push(syn::Item::Fn(func));
                }
                // Check for #[init]
                else if has_bare_attr(&func.attrs, "init") {
                    func.attrs.retain(|a| !a.path().is_ident("init"));
                    let fn_name = func.sig.ident.to_string();

                    if init_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "only one #[init] function is allowed",
                        ));
                    }
                    init_fn = Some(fn_name.clone());

                    // Rename the original to an inner function, generate the extern wrapper
                    let inner_ident =
                        syn::Ident::new(&format!("__{fn_name}_inner"), func.sig.ident.span());
                    func.sig.ident = inner_ident.clone();
                    transformed_items.push(syn::Item::Fn(func));

                    // Generate qimen_plugin_init that calls the inner function
                    let init_wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn qimen_plugin_init(
                            config: ::abi_stable_host_api::PluginInitConfig,
                        ) -> ::abi_stable_host_api::PluginInitResult {
                            #inner_ident(config)
                        }
                    };
                    transformed_items.push(init_wrapper);
                    continue;
                }
                // Check for #[shutdown]
                else if has_bare_attr(&func.attrs, "shutdown") {
                    func.attrs.retain(|a| !a.path().is_ident("shutdown"));
                    let fn_name = func.sig.ident.to_string();

                    if shutdown_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "only one #[shutdown] function is allowed",
                        ));
                    }
                    shutdown_fn = Some(fn_name.clone());

                    let inner_ident =
                        syn::Ident::new(&format!("__{fn_name}_inner"), func.sig.ident.span());
                    func.sig.ident = inner_ident.clone();
                    transformed_items.push(syn::Item::Fn(func));

                    let shutdown_wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn qimen_plugin_shutdown() {
                            #inner_ident()
                        }
                    };
                    transformed_items.push(shutdown_wrapper);
                    continue;
                }
                // Check for #[pre_handle]
                else if has_bare_attr(&func.attrs, "pre_handle") {
                    func.attrs.retain(|a| !a.path().is_ident("pre_handle"));
                    let fn_name = func.sig.ident.to_string();

                    if pre_handle_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "only one #[pre_handle] function is allowed",
                        ));
                    }
                    pre_handle_fn = Some(fn_name.clone());

                    let inner_ident =
                        syn::Ident::new(&format!("__{fn_name}_inner"), func.sig.ident.span());
                    func.sig.ident = inner_ident.clone();
                    transformed_items.push(syn::Item::Fn(func));

                    let wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn qimen_plugin_pre_handle(
                            req: &::abi_stable_host_api::InterceptorRequest,
                        ) -> ::abi_stable_host_api::InterceptorResponse {
                            #inner_ident(req)
                        }
                    };
                    transformed_items.push(wrapper);
                    continue;
                }
                // Check for #[after_completion]
                else if has_bare_attr(&func.attrs, "after_completion") {
                    func.attrs.retain(|a| !a.path().is_ident("after_completion"));
                    let fn_name = func.sig.ident.to_string();

                    if after_completion_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "only one #[after_completion] function is allowed",
                        ));
                    }
                    after_completion_fn = Some(fn_name.clone());

                    let inner_ident =
                        syn::Ident::new(&format!("__{fn_name}_inner"), func.sig.ident.span());
                    func.sig.ident = inner_ident.clone();
                    transformed_items.push(syn::Item::Fn(func));

                    let wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn qimen_plugin_after_completion(
                            req: &::abi_stable_host_api::InterceptorRequest,
                        ) {
                            #inner_ident(req)
                        }
                    };
                    transformed_items.push(wrapper);
                    continue;
                } else {
                    // Pass through unchanged
                    transformed_items.push(syn::Item::Fn(func));
                }
            }
            other => transformed_items.push(other),
        }
    }

    // Generate the descriptor function
    let plugin_id = &args.id;
    let plugin_version = &args.version;

    let command_registrations: Vec<TokenStream2> = command_entries
        .iter()
        .map(|(fn_name, cmd)| {
            let name = &cmd.name;
            let description = &cmd.description;
            let callback = fn_name;
            let aliases = &cmd.aliases;
            let category = &cmd.category;
            let role = &cmd.role;
            let scope = &cmd.scope;

            quote! {
                .add_command_full(::abi_stable_host_api::CommandDescriptorEntry {
                    name: ::abi_stable::std_types::RString::from(#name),
                    description: ::abi_stable::std_types::RString::from(#description),
                    callback_symbol: ::abi_stable::std_types::RString::from(#callback),
                    aliases: ::abi_stable::std_types::RString::from(#aliases),
                    category: ::abi_stable::std_types::RString::from(#category),
                    required_role: ::abi_stable::std_types::RString::from(#role),
                    scope: ::abi_stable::std_types::RString::from(#scope),
                })
            }
        })
        .collect();

    let route_registrations: Vec<TokenStream2> = route_entries
        .iter()
        .map(|(fn_name, route)| {
            let kind = &route.kind;
            let events = &route.events;
            let callback = fn_name;

            quote! {
                .add_route(#kind, #events, #callback)
            }
        })
        .collect();

    // Generate interceptor registration if any interceptor hooks are present
    let interceptor_registration = if pre_handle_fn.is_some() || after_completion_fn.is_some() {
        let pre_sym = if pre_handle_fn.is_some() {
            "qimen_plugin_pre_handle"
        } else {
            ""
        };
        let after_sym = if after_completion_fn.is_some() {
            "qimen_plugin_after_completion"
        } else {
            ""
        };
        quote! {
            .add_interceptor(#pre_sym, #after_sym)
        }
    } else {
        quote! {}
    };

    let output = quote! {
        #(#mod_attrs)*
        #mod_vis mod #mod_name {
            #(#transformed_items)*
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn qimen_plugin_descriptor() -> ::abi_stable_host_api::PluginDescriptor {
            ::abi_stable_host_api::PluginDescriptor::new(#plugin_id, #plugin_version)
                #(#command_registrations)*
                #(#route_registrations)*
                #interceptor_registration
        }
    };

    Ok(output)
}

/// Make a function `pub unsafe extern "C"` with `#[unsafe(no_mangle)]`.
fn make_extern_c(func: &mut syn::ItemFn) {
    func.vis = syn::parse_quote!(pub);
    func.sig.unsafety = Some(syn::parse_quote!(unsafe));
    func.sig.abi = Some(syn::parse_quote!(extern "C"));

    let no_mangle_attr: syn::Attribute = syn::parse_quote!(#[unsafe(no_mangle)]);
    func.attrs.insert(0, no_mangle_attr);
}

/// Extract an attribute by name that has parenthesized arguments, e.g. `#[command(...)]`.
/// Returns the token stream inside the parens and the remaining attributes.
fn extract_attr(
    attrs: &[syn::Attribute],
    name: &str,
) -> syn::Result<Option<(TokenStream2, Vec<syn::Attribute>)>> {
    let mut found_tokens = None;
    let mut remaining = Vec::new();

    for attr in attrs {
        if attr.path().is_ident(name) {
            let tokens = attr.parse_args::<proc_macro2::TokenStream>()?;
            found_tokens = Some(tokens);
        } else {
            remaining.push(attr.clone());
        }
    }

    Ok(found_tokens.map(|t| (t, remaining)))
}

/// Check if a bare attribute (no arguments) exists, e.g. `#[init]`.
fn has_bare_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}
