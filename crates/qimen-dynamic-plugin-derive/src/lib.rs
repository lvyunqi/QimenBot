use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{
    Ident, ItemMod, LitInt, LitStr, Token,
    parse::{Parse, ParseStream},
    parse_macro_input,
};

// ─── Plugin-level args ──────────────────────────────────────────────────

// Parse: id/version/api plus optional API 0.6 configuration contract.
struct PluginArgs {
    id: String,
    version: String,
    api: String,
    config_schema: Option<String>,
    config_ui: Option<String>,
    config_version: u32,
    config_apply: String,
}

impl Parse for PluginArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut id = None;
        let mut version = None;
        let mut api = None;
        let mut config_schema = None;
        let mut config_ui = None;
        let mut config_version = None;
        let mut config_apply = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let key_name = key.to_string();

            if key_name == "config_version" {
                let value: LitInt = input.parse()?;
                config_version = Some(value.base10_parse::<u32>()?);
            } else {
                let value: LitStr = input.parse()?;
                match key_name.as_str() {
                    "id" => id = Some(value.value()),
                    "version" => version = Some(value.value()),
                    "api" => api = Some(value.value()),
                    "config_schema" => config_schema = Some(value.value()),
                    "config_ui" => config_ui = Some(value.value()),
                    "config_apply" => config_apply = Some(value.value()),
                    other => {
                        return Err(syn::Error::new(key.span(), format!("unknown key: {other}")));
                    }
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let api = api.unwrap_or_else(|| "0.3".to_string());
        if !matches!(api.as_str(), "0.1" | "0.2" | "0.3" | "0.4" | "0.5" | "0.6") {
            return Err(input.error("api must be one of 0.1 through 0.6"));
        }
        let config_requested = config_schema.is_some()
            || config_ui.is_some()
            || config_version.is_some()
            || config_apply.is_some();
        if config_requested && api != "0.6" {
            return Err(input.error("plugin configuration requires dynamic plugin API 0.6"));
        }
        if config_schema.is_none()
            && (config_ui.is_some() || config_version.is_some() || config_apply.is_some())
        {
            return Err(input.error("config_schema is required when config options are declared"));
        }
        let config_version = config_version.unwrap_or(1);
        if config_version == 0 {
            return Err(input.error("config_version must be greater than zero"));
        }
        let config_apply = config_apply.unwrap_or_else(|| "reload".to_string());
        if !matches!(config_apply.as_str(), "live" | "reload" | "restart") {
            return Err(input.error("config_apply must be live, reload, or restart"));
        }

        Ok(PluginArgs {
            id: id.ok_or_else(|| input.error("missing `id`"))?,
            version: version.ok_or_else(|| input.error("missing `version`"))?,
            api,
            config_schema,
            config_ui,
            config_version,
            config_apply,
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
                other => return Err(syn::Error::new(key.span(), format!("unknown key: {other}"))),
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
                other => return Err(syn::Error::new(key.span(), format!("unknown key: {other}"))),
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

// Parse: method = "POST", path = "/events"
struct WebhookArgs {
    method: String,
    path: String,
}

impl Parse for WebhookArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut method = None;
        let mut path = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "method" => method = Some(value.value()),
                "path" => path = Some(value.value()),
                other => return Err(syn::Error::new(key.span(), format!("unknown key: {other}"))),
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let method = method
            .ok_or_else(|| input.error("missing `method`"))?
            .trim()
            .to_ascii_uppercase();
        if method.is_empty() || !method.bytes().all(|byte| byte.is_ascii_uppercase()) {
            return Err(input.error("webhook method must contain only ASCII letters"));
        }

        let path = path.ok_or_else(|| input.error("missing `path`"))?;
        if !path.starts_with('/')
            || path.contains('?')
            || path.contains('#')
            || path.contains('*')
            || path.contains("//")
            || path.split('/').any(|segment| matches!(segment, "." | ".."))
        {
            return Err(input.error(
                "webhook path must be an exact absolute path without traversal, query, fragment, or wildcard",
            ));
        }

        Ok(Self { method, path })
    }
}

// ─── Macro entry point ──────────────────────────────────────────────────

/// Attribute macro for declaring a dynamic plugin module.
///
/// Usage:
/// ```ignore
/// #[dynamic_plugin(
///     id = "my-plugin",
///     version = "0.1.0",
///     api = "0.6",
///     config_schema = "config.schema.json",
///     config_ui = "config.ui.json",
///     config_apply = "reload",
///     config_version = 1,
/// )]
/// mod my_plugin {
///     #[command(name = "greet", description = "Say hello", aliases = "hi,hello")]
///     fn greet(req: &CommandRequest) -> CommandResponse {
///         CommandResponse::text("Hello!")
///     }
///
///     #[route(kind = "notice", events = "GroupPoke,PrivatePoke")]
///     fn on_poke(req: &NoticeRequest) -> NoticeResponse { ... }
///
///     #[webhook(method = "POST", path = "/events")]
///     fn events(req: &WebhookRequest) -> WebhookResponse { ... }
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
    let mut webhook_entries = Vec::new();
    let mut init_fn: Option<String> = None;
    let mut shutdown_fn: Option<String> = None;
    let mut pre_handle_fn: Option<String> = None;
    let mut after_completion_fn: Option<String> = None;
    let mut validate_config_fn: Option<String> = None;
    let mut config_change_fn: Option<String> = None;
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
                // Check for #[webhook(...)]
                else if let Some((webhook_tokens, remaining_attrs)) =
                    extract_attr(&func.attrs, "webhook")?
                {
                    if !matches!(args.api.as_str(), "0.5" | "0.6") {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "#[webhook] requires dynamic plugin API 0.5 or 0.6",
                        ));
                    }
                    let webhook_args: WebhookArgs = syn::parse2(webhook_tokens)?;
                    func.attrs = remaining_attrs;
                    let fn_name = func.sig.ident.to_string();
                    let export_ident = func.sig.ident.clone();
                    let inner_ident = syn::Ident::new(
                        &format!("__{fn_name}_webhook_inner"),
                        func.sig.ident.span(),
                    );
                    func.sig.ident = inner_ident.clone();
                    webhook_entries.push((fn_name, webhook_args));
                    transformed_items.push(syn::Item::Fn(func));

                    // Never allow a Rust panic to cross an `extern "C"` boundary.
                    // The host can turn this stable fallback into an HTTP 500 response.
                    let webhook_wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn #export_ident(
                            req: &::abi_stable_host_api::WebhookRequest,
                        ) -> ::abi_stable_host_api::WebhookResponse {
                            match ::std::panic::catch_unwind(
                                ::std::panic::AssertUnwindSafe(|| #inner_ident(req)),
                            ) {
                                Ok(response) => response,
                                Err(_) => ::abi_stable_host_api::WebhookResponse::text(
                                    500,
                                    "webhook callback panicked",
                                ),
                            }
                        }
                    };
                    transformed_items.push(webhook_wrapper);
                }
                // API 0.6 配置校验回调不能修改插件状态。
                else if has_bare_attr(&func.attrs, "validate_config") {
                    func.attrs
                        .retain(|attribute| !attribute.path().is_ident("validate_config"));
                    if args.api != "0.6" || args.config_schema.is_none() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "#[validate_config] requires API 0.6 and config_schema",
                        ));
                    }
                    if validate_config_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "only one #[validate_config] function is allowed",
                        ));
                    }
                    let fn_name = func.sig.ident.to_string();
                    validate_config_fn = Some(fn_name.clone());
                    let inner_ident =
                        syn::Ident::new(&format!("__{fn_name}_inner"), func.sig.ident.span());
                    func.sig.ident = inner_ident.clone();
                    transformed_items.push(syn::Item::Fn(func));
                    let wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn qimen_plugin_validate_config_v1(
                            request: &::abi_stable_host_api::PluginConfigRequest,
                        ) -> ::abi_stable_host_api::PluginConfigResult {
                            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                                #inner_ident(request)
                            })) {
                                Ok(result) => result,
                                Err(_) => ::abi_stable_host_api::PluginConfigResult::err(
                                    "plugin config validation panicked",
                                ),
                            }
                        }
                    };
                    transformed_items.push(wrapper);
                    continue;
                }
                // 只有声明 live 的插件会在保存后收到即时应用回调。
                else if has_bare_attr(&func.attrs, "config_change") {
                    func.attrs
                        .retain(|attribute| !attribute.path().is_ident("config_change"));
                    if args.api != "0.6" || args.config_schema.is_none() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "#[config_change] requires API 0.6 and config_schema",
                        ));
                    }
                    if config_change_fn.is_some() {
                        return Err(syn::Error::new_spanned(
                            &func.sig.ident,
                            "only one #[config_change] function is allowed",
                        ));
                    }
                    let fn_name = func.sig.ident.to_string();
                    config_change_fn = Some(fn_name.clone());
                    let inner_ident =
                        syn::Ident::new(&format!("__{fn_name}_inner"), func.sig.ident.span());
                    func.sig.ident = inner_ident.clone();
                    transformed_items.push(syn::Item::Fn(func));
                    let wrapper: syn::Item = syn::parse_quote! {
                        #[unsafe(no_mangle)]
                        pub unsafe extern "C" fn qimen_plugin_apply_config_v1(
                            request: &::abi_stable_host_api::PluginConfigRequest,
                        ) -> ::abi_stable_host_api::PluginConfigResult {
                            match ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                                #inner_ident(request)
                            })) {
                                Ok(result) => result,
                                Err(_) => ::abi_stable_host_api::PluginConfigResult::err(
                                    "plugin config apply panicked",
                                ),
                            }
                        }
                    };
                    transformed_items.push(wrapper);
                    continue;
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
                    func.attrs
                        .retain(|a| !a.path().is_ident("after_completion"));
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

    if args.config_apply == "live" && config_change_fn.is_none() {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "config_apply = \"live\" requires one #[config_change] function",
        ));
    }
    if args.config_apply != "live" && config_change_fn.is_some() {
        return Err(syn::Error::new_spanned(
            &module.ident,
            "#[config_change] is only used with config_apply = \"live\"",
        ));
    }

    // Generate the descriptor function
    let plugin_id = &args.id;
    let plugin_version = &args.version;
    let plugin_api = &args.api;

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

    let webhook_registrations: Vec<TokenStream2> = webhook_entries
        .iter()
        .map(|(fn_name, webhook)| {
            let method = &webhook.method;
            let path = &webhook.path;
            let callback = fn_name;
            quote! {
                ::abi_stable_host_api::WebhookDescriptorEntry {
                    method: ::abi_stable::std_types::RString::from(#method),
                    path: ::abi_stable::std_types::RString::from(#path),
                    callback_symbol: ::abi_stable::std_types::RString::from(#callback),
                }
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

    let host_api_exports = if matches!(args.api.as_str(), "0.4" | "0.5" | "0.6") {
        quote! {
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn qimen_plugin_bind_host_api_v1(
                api: *const ::abi_stable_host_api::HostApiV1,
            ) -> i32 {
                unsafe { ::abi_stable_host_api::bind_host_api_v1(api) }
            }

            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn qimen_plugin_unbind_host_api_v1() -> i32 {
                ::abi_stable_host_api::unbind_host_api_v1()
            }
        }
    } else {
        quote! {}
    };

    let webhook_descriptor_export = if matches!(args.api.as_str(), "0.5" | "0.6") {
        quote! {
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn qimen_plugin_webhook_descriptors_v1(
            ) -> ::abi_stable::std_types::RVec<::abi_stable_host_api::WebhookDescriptorEntry> {
                vec![#(#webhook_registrations),*].into_iter().collect()
            }
        }
    } else {
        quote! {}
    };

    let config_descriptor_export = if let Some(schema_path) = &args.config_schema {
        let config_version = args.config_version;
        let config_apply = &args.config_apply;
        let ui_schema = args
            .config_ui
            .as_ref()
            .map(|path| quote! { include_str!(#path) })
            .unwrap_or_else(|| quote! { "" });
        quote! {
            #[unsafe(no_mangle)]
            pub unsafe extern "C" fn qimen_plugin_config_descriptor_v1(
            ) -> ::abi_stable_host_api::PluginConfigDescriptorV1 {
                ::abi_stable_host_api::PluginConfigDescriptorV1::new(
                    #config_version,
                    #config_apply,
                    include_str!(#schema_path),
                    #ui_schema,
                )
            }
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
                .with_api_version(#plugin_api)
                #(#command_registrations)*
                #(#route_registrations)*
                #interceptor_registration
        }

        #host_api_exports
        #webhook_descriptor_export
        #config_descriptor_export

        /// Drain all queued `SendAction`s produced by `BotApi` / `SendBuilder`
        /// during the most recent FFI callback.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn qimen_plugin_flush_sends() -> ::abi_stable::std_types::RVec<::abi_stable_host_api::SendAction> {
            ::abi_stable_host_api::drain_send_queue()
                .into_iter()
                .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn expand(args: &str) -> String {
        let args = syn::parse_str::<PluginArgs>(args).expect("plugin args");
        let module = syn::parse_str::<ItemMod>(
            r#"
            mod fixture {
                #[command(name = "ping", description = "reply")]
                fn ping(_req: &CommandRequest) -> CommandResponse {
                    CommandResponse::text("pong")
                }
            }
            "#,
        )
        .expect("module");
        expand_dynamic_plugin(args, module)
            .expect("expand")
            .to_string()
    }

    #[test]
    fn omitted_api_keeps_legacy_03_descriptor_without_host_exports() {
        let output = expand(r#"id = "fixture", version = "0.1.0""#);
        assert!(output.contains("with_api_version (\"0.3\")"));
        assert!(!output.contains("qimen_plugin_bind_host_api_v1"));
        assert!(!output.contains("qimen_plugin_unbind_host_api_v1"));
    }

    #[test]
    fn api_04_generates_bind_and_unbind_exports() {
        let output = expand(r#"id = "fixture", version = "0.1.0", api = "0.4""#);
        assert!(output.contains("with_api_version (\"0.4\")"));
        assert!(output.contains("qimen_plugin_bind_host_api_v1"));
        assert!(output.contains("qimen_plugin_unbind_host_api_v1"));
    }

    #[test]
    fn api_05_generates_webhook_descriptors_and_host_exports() {
        let args =
            syn::parse_str::<PluginArgs>(r#"id = "fixture", version = "0.1.0", api = "0.5""#)
                .expect("plugin args");
        let module = syn::parse_str::<ItemMod>(
            r#"
            mod fixture {
                #[webhook(method = "post", path = "/events")]
                fn events(_req: &WebhookRequest) -> WebhookResponse {
                    WebhookResponse::text(200, "ok")
                }
            }
            "#,
        )
        .expect("module");
        let output = expand_dynamic_plugin(args, module)
            .expect("expand")
            .to_string();

        assert!(output.contains("with_api_version"));
        assert!(output.contains("0.5"));
        assert!(output.contains("qimen_plugin_webhook_descriptors_v1"));
        assert!(output.contains("WebhookDescriptorEntry"));
        assert!(output.contains("catch_unwind"));
        assert!(output.contains("POST"));
        assert!(output.contains("/events"));
        assert!(output.contains("qimen_plugin_bind_host_api_v1"));
    }

    #[test]
    fn webhook_requires_api_05_or_newer() {
        let args =
            syn::parse_str::<PluginArgs>(r#"id = "fixture", version = "0.1.0", api = "0.4""#)
                .expect("plugin args");
        let module = syn::parse_str::<ItemMod>(
            r#"
            mod fixture {
                #[webhook(method = "POST", path = "/events")]
                fn events(_req: &WebhookRequest) -> WebhookResponse {
                    WebhookResponse::text(200, "ok")
                }
            }
            "#,
        )
        .expect("module");

        assert!(expand_dynamic_plugin(args, module).is_err());
    }

    #[test]
    fn unsupported_api_is_rejected() {
        let result =
            syn::parse_str::<PluginArgs>(r#"id = "fixture", version = "0.1.0", api = "0.7""#);
        assert!(result.is_err());
    }

    #[test]
    fn api_06_generates_config_descriptor_validation_and_live_apply() {
        let args = syn::parse_str::<PluginArgs>(
            r#"id = "fixture", version = "0.1.0", api = "0.6", config_schema = "config.schema.json", config_ui = "config.ui.json", config_apply = "live", config_version = 2"#,
        )
        .expect("plugin args");
        let module = syn::parse_str::<ItemMod>(
            r#"
            mod fixture {
                #[validate_config]
                fn validate(_req: &PluginConfigRequest) -> PluginConfigResult {
                    PluginConfigResult::ok()
                }

                #[config_change]
                fn apply(_req: &PluginConfigRequest) -> PluginConfigResult {
                    PluginConfigResult::ok()
                }
            }
            "#,
        )
        .expect("module");
        let output = expand_dynamic_plugin(args, module)
            .expect("expand")
            .to_string();

        assert!(output.contains("qimen_plugin_config_descriptor_v1"));
        assert!(output.contains("qimen_plugin_validate_config_v1"));
        assert!(output.contains("qimen_plugin_apply_config_v1"));
        assert!(output.contains("config.schema.json"));
        assert!(output.contains("config.ui.json"));
        assert!(output.contains("PluginConfigDescriptorV1"));
    }

    #[test]
    fn live_config_requires_apply_callback() {
        let args = syn::parse_str::<PluginArgs>(
            r#"id = "fixture", version = "0.1.0", api = "0.6", config_schema = "config.schema.json", config_apply = "live""#,
        )
        .expect("plugin args");
        let module = syn::parse_str::<ItemMod>("mod fixture {}").expect("module");
        assert!(expand_dynamic_plugin(args, module).is_err());
    }

    #[test]
    fn config_contract_requires_api_06() {
        let result = syn::parse_str::<PluginArgs>(
            r#"id = "fixture", version = "0.1.0", api = "0.5", config_schema = "config.schema.json""#,
        );
        assert!(result.is_err());
    }
}
