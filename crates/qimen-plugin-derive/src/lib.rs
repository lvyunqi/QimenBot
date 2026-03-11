use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Ident, ItemFn, ItemImpl, ItemStruct, LitStr, Token};

/// Marks a function as a plugin entry point.
/// The function should return a type that implements `Module`.
///
/// Usage:
/// ```rust,ignore
/// #[qimen_plugin(id = "my-plugin")]
/// fn my_plugin() -> MyPluginModule {
///     MyPluginModule::new()
/// }
/// ```
///
/// This generates both the original function and a `register_<fn_name>` helper
/// that wraps the return value in `Box<dyn Module>`.
#[deprecated(note = "Use #[qimen_module] + #[qimen_commands] instead")]
#[proc_macro_attribute]
pub fn qimen_plugin(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let fn_block = &input.block;
    let fn_vis = &input.vis;
    let fn_return = &input.sig.output;

    // Parse the id attribute
    let attr_str = attr.to_string();
    let _plugin_id = attr_str
        .split('=')
        .nth(1)
        .map(|s| s.trim().trim_matches('"').to_string())
        .unwrap_or_else(|| fn_name.to_string());

    let register_fn_name = syn::Ident::new(&format!("register_{fn_name}"), fn_name.span());

    let expanded = quote! {
        #fn_vis fn #fn_name() #fn_return #fn_block

        /// Auto-generated registration function for the plugin module.
        #fn_vis fn #register_fn_name() -> Box<dyn qimen_plugin_api::Module> {
            Box::new(#fn_name())
        }
    };

    expanded.into()
}

/// Derive macro for creating a simple command plugin from a struct.
/// Requires the struct to have `metadata()` and `commands()` defined.
///
/// Usage:
/// ```rust,ignore
/// #[derive(CommandPluginDerive)]
/// #[plugin_meta(id = "my-cmd", name = "My Command", version = "0.1.0")]
/// struct MyCommand;
/// ```
#[deprecated(note = "Use #[qimen_module] + #[qimen_commands] instead")]
#[proc_macro_derive(CommandPluginDerive, attributes(plugin_meta))]
pub fn derive_command_plugin(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    let name = &input.ident;

    // Parse plugin_meta attribute
    let mut plugin_id = String::new();
    let mut plugin_name = String::new();
    let mut plugin_version = "0.1.0".to_string();
    let mut plugin_description = String::new();

    for attr in &input.attrs {
        if attr.path().is_ident("plugin_meta") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("id") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    plugin_id = lit.value();
                } else if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    plugin_name = lit.value();
                } else if meta.path.is_ident("version") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    plugin_version = lit.value();
                } else if meta.path.is_ident("description") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    plugin_description = lit.value();
                }
                Ok(())
            });
        }
    }

    if plugin_id.is_empty() {
        plugin_id = name.to_string().to_lowercase();
    }
    if plugin_name.is_empty() {
        plugin_name = name.to_string();
    }

    let expanded = quote! {
        impl #name {
            /// Auto-generated metadata from derive macro attributes.
            pub fn derived_metadata(&self) -> qimen_plugin_api::PluginMetadata {
                qimen_plugin_api::PluginMetadata {
                    id: #plugin_id,
                    name: #plugin_name,
                    version: #plugin_version,
                    description: #plugin_description,
                    api_version: "0.1",
                    compatibility: qimen_plugin_api::PluginCompatibility {
                        host_api: "0.1",
                        framework_min: "0.1.0",
                        framework_max: "0.1.x",
                    },
                }
            }
        }
    };

    expanded.into()
}

// ─── New macro infrastructure ───────────────────────────────────────────

/// Parsed arguments for `#[qimen_module(...)]`.
struct ModuleArgs {
    id: String,
    version: String,
    name: Option<String>,
    description: String,
    system_plugins: Vec<Ident>,
    interceptors: Vec<Ident>,
}

impl Parse for ModuleArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut id = None;
        let mut version = "0.1.0".to_string();
        let mut name = None;
        let mut description = String::new();
        let mut system_plugins = Vec::new();
        let mut interceptors = Vec::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let _eq: Token![=] = input.parse()?;

            match key.to_string().as_str() {
                "id" => {
                    let lit: LitStr = input.parse()?;
                    id = Some(lit.value());
                }
                "version" => {
                    let lit: LitStr = input.parse()?;
                    version = lit.value();
                }
                "name" => {
                    let lit: LitStr = input.parse()?;
                    name = Some(lit.value());
                }
                "description" => {
                    let lit: LitStr = input.parse()?;
                    description = lit.value();
                }
                "system_plugins" => {
                    let content;
                    syn::bracketed!(content in input);
                    let plugins: Punctuated<Ident, Token![,]> =
                        content.parse_terminated(Ident::parse, Token![,])?;
                    system_plugins = plugins.into_iter().collect();
                }
                "interceptors" => {
                    let content;
                    syn::bracketed!(content in input);
                    let items: Punctuated<Ident, Token![,]> =
                        content.parse_terminated(Ident::parse, Token![,])?;
                    interceptors = items.into_iter().collect();
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown #[module] attribute: `{other}`"),
                    ));
                }
            }

            // consume optional trailing comma
            let _ = input.parse::<Token![,]>();
        }

        let id =
            id.ok_or_else(|| syn::Error::new(Span::call_site(), "#[module] requires `id`"))?;

        Ok(ModuleArgs {
            id,
            version,
            name,
            description,
            system_plugins,
            interceptors,
        })
    }
}

/// Marks a struct as a QimenBot module, generating hidden constants and a
/// fallback `Module` impl (overridden when `#[qimen_commands]` is also used).
///
/// # Usage
/// ```rust,ignore
/// #[qimen_module(id = "my-plugin", version = "0.1.0", system_plugins = [MySysPlugin])]
/// pub struct MyPlugin;
/// ```
///
/// ## Supported attributes
/// - `id` (required) — unique module id
/// - `version` (default `"0.1.0"`)
/// - `name` (default = struct name)
/// - `description` (default `""`)
/// - `system_plugins` (optional) — list of `SystemPlugin` types to instantiate
#[proc_macro_attribute]
pub fn qimen_module(attr: TokenStream, item: TokenStream) -> TokenStream {
    module_impl(attr, item)
}

/// Short-name alias for `#[qimen_module]`.
#[proc_macro_attribute]
pub fn module(attr: TokenStream, item: TokenStream) -> TokenStream {
    module_impl(attr, item)
}

fn module_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as ModuleArgs);

    // Try parsing as ItemStruct first, then as ItemImpl
    let item2: proc_macro2::TokenStream = item.clone().into();
    if let Ok(input) = syn::parse::<ItemStruct>(item) {
        // Applied to a struct — original behaviour
        qimen_module_for_struct(args, input)
    } else if let Ok(input) = syn::parse2::<ItemImpl>(item2) {
        // Applied to an impl block — generate struct + hidden constants, pass impl through
        qimen_module_for_impl(args, input)
    } else {
        syn::Error::new(
            Span::call_site(),
            "#[module] must be applied to a struct or an impl block",
        )
        .to_compile_error()
        .into()
    }
}

fn qimen_module_for_struct(args: ModuleArgs, input: ItemStruct) -> TokenStream {
    let struct_name = &input.ident;
    let hidden_impl = gen_module_hidden_impl(struct_name, &args);

    let expanded = quote! {
        #input
        #hidden_impl
    };
    expanded.into()
}

fn qimen_module_for_impl(args: ModuleArgs, input: ItemImpl) -> TokenStream {
    // Extract struct name from impl's self type
    let struct_name = match input.self_ty.as_ref() {
        syn::Type::Path(tp) => match tp.path.get_ident() {
            Some(ident) => ident.clone(),
            None => {
                return syn::Error::new_spanned(
                    &input.self_ty,
                    "#[qimen_module] on impl: cannot extract type name",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.self_ty,
                "#[qimen_module] on impl: expected a simple type path",
            )
            .to_compile_error()
            .into();
        }
    };

    let hidden_impl = gen_module_hidden_impl(&struct_name, &args);

    let expanded = quote! {
        // Auto-generated struct
        pub struct #struct_name;

        #hidden_impl

        // Pass the impl block through (will be processed by #[qimen_commands])
        #input
    };
    expanded.into()
}

fn gen_module_hidden_impl(struct_name: &Ident, args: &ModuleArgs) -> proc_macro2::TokenStream {
    let mod_id = &args.id;
    let mod_version = &args.version;
    let mod_name = args
        .name
        .as_deref()
        .unwrap_or(&struct_name.to_string())
        .to_string();
    let mod_description = &args.description;

    let sys_plugin_exprs: Vec<_> = args
        .system_plugins
        .iter()
        .map(|ty| {
            quote! { std::sync::Arc::new(#ty) as std::sync::Arc<dyn qimen_plugin_api::SystemPlugin> }
        })
        .collect();

    let interceptor_exprs: Vec<_> = args
        .interceptors
        .iter()
        .map(|ty| {
            quote! { std::sync::Arc::new(#ty) as std::sync::Arc<dyn qimen_plugin_api::MessageEventInterceptor> }
        })
        .collect();

    quote! {
        #[doc(hidden)]
        impl #struct_name {
            #[doc(hidden)]
            pub const __QIMEN_MODULE_ID: &'static str = #mod_id;
            #[doc(hidden)]
            pub const __QIMEN_MODULE_VERSION: &'static str = #mod_version;
            #[doc(hidden)]
            pub const __QIMEN_MODULE_NAME: &'static str = #mod_name;
            #[doc(hidden)]
            pub const __QIMEN_MODULE_DESCRIPTION: &'static str = #mod_description;

            #[doc(hidden)]
            pub fn __qimen_system_plugins() -> Vec<std::sync::Arc<dyn qimen_plugin_api::SystemPlugin>> {
                vec![#(#sys_plugin_exprs),*]
            }

            #[doc(hidden)]
            pub fn __qimen_interceptors() -> Vec<std::sync::Arc<dyn qimen_plugin_api::MessageEventInterceptor>> {
                vec![#(#interceptor_exprs),*]
            }
        }
    }
}

// ─── #[qimen_commands] ─────────────────────────────────────────────────

/// Parsed arguments for a `#[command(...)]` attribute on a method.
struct CommandArgs {
    name: String,
    desc: String,
    aliases: Vec<String>,
    examples: Vec<String>,
    category: Option<String>,
    hidden: bool,
    role: Option<String>,
}

/// Custom parser for command attributes that supports both positional and named forms:
/// - `#[command("desc")]` — name inferred from method_name
/// - `#[command("desc", aliases = ["a"])]` — positional desc + named extras
/// - `#[command(name = "x", desc = "y")]` — fully named (backward compat)
struct CommandAttrContent {
    positional_desc: Option<String>,
    name: Option<String>,
    desc: Option<String>,
    aliases: Vec<String>,
    examples: Vec<String>,
    category: Option<String>,
    hidden: bool,
    role: Option<String>,
}

impl Parse for CommandAttrContent {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = CommandAttrContent {
            positional_desc: None,
            name: None,
            desc: None,
            aliases: Vec::new(),
            examples: Vec::new(),
            category: None,
            hidden: false,
            role: None,
        };

        // Try to parse a leading string literal (positional desc)
        if input.peek(LitStr) {
            let lit: LitStr = input.parse()?;
            result.positional_desc = Some(lit.value());
            let _ = input.parse::<Token![,]>(); // consume optional comma
        }

        // Parse remaining key = value pairs
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "hidden" => {
                    result.hidden = true;
                }
                "name" => {
                    let _eq: Token![=] = input.parse()?;
                    let lit: LitStr = input.parse()?;
                    result.name = Some(lit.value());
                }
                "desc" => {
                    let _eq: Token![=] = input.parse()?;
                    let lit: LitStr = input.parse()?;
                    result.desc = Some(lit.value());
                }
                "aliases" => {
                    let _eq: Token![=] = input.parse()?;
                    let content;
                    syn::bracketed!(content in input);
                    let lits: Punctuated<LitStr, Token![,]> =
                        content.parse_terminated(|i: ParseStream| i.parse::<LitStr>(), Token![,])?;
                    result.aliases = lits.iter().map(|l| l.value()).collect();
                }
                "examples" => {
                    let _eq: Token![=] = input.parse()?;
                    let content;
                    syn::bracketed!(content in input);
                    let lits: Punctuated<LitStr, Token![,]> =
                        content.parse_terminated(|i: ParseStream| i.parse::<LitStr>(), Token![,])?;
                    result.examples = lits.iter().map(|l| l.value()).collect();
                }
                "category" => {
                    let _eq: Token![=] = input.parse()?;
                    let lit: LitStr = input.parse()?;
                    result.category = Some(lit.value());
                }
                "role" => {
                    let _eq: Token![=] = input.parse()?;
                    let lit: LitStr = input.parse()?;
                    result.role = Some(lit.value());
                }
                other => {
                    return Err(syn::Error::new(
                        key.span(),
                        format!("unknown #[command] attribute: `{other}`"),
                    ));
                }
            }
            let _ = input.parse::<Token![,]>(); // consume optional trailing comma
        }

        Ok(result)
    }
}

fn parse_command_attr(attr: &syn::Attribute, method_name: &str) -> syn::Result<CommandArgs> {
    let content: CommandAttrContent = attr.parse_args()?;

    // Resolve desc: positional takes priority, then named `desc`
    let desc = content
        .positional_desc
        .or(content.desc)
        .ok_or_else(|| syn::Error::new_spanned(attr, "#[command] requires a description"))?;

    // Resolve name: named `name` takes priority, otherwise infer from method_name
    let name = content
        .name
        .unwrap_or_else(|| method_name.replace('_', "-"));

    Ok(CommandArgs {
        name,
        desc,
        aliases: content.aliases,
        examples: content.examples,
        category: content.category,
        hidden: content.hidden,
        role: content.role,
    })
}

/// Parsed arguments for `#[qimen_commands(ModuleName)]` (optional).
struct CommandsMacroArgs {
    module_name: Option<Ident>,
}

impl Parse for CommandsMacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            Ok(CommandsMacroArgs { module_name: None })
        } else {
            let module_name: Ident = input.parse()?;
            Ok(CommandsMacroArgs {
                module_name: Some(module_name),
            })
        }
    }
}

/// Transforms an `impl` block into a fully wired `CommandPlugin` + `Module` implementation.
///
/// # Usage
/// ```rust,ignore
/// #[qimen_commands(MyPlugin)]
/// impl MyPlugin {
///     #[command(name = "echo", desc = "Echo message")]
///     async fn echo(&self, ctx: &CommandPluginContext<'_>, args: Vec<String>) -> CommandPluginSignal {
///         CommandPluginSignal::Reply(Message::text("echo"))
///     }
/// }
/// ```
///
/// The macro generates:
/// 1. The original `impl` block (with `#[command]` attrs stripped)
/// 2. A hidden unit struct that implements `CommandPlugin`, routing `on_command`
///    to the annotated methods
/// 3. A `Module` impl for the annotated struct
#[proc_macro_attribute]
pub fn qimen_commands(attr: TokenStream, item: TokenStream) -> TokenStream {
    commands_impl(attr, item)
}

/// Short-name alias for `#[qimen_commands]`.
#[proc_macro_attribute]
pub fn commands(attr: TokenStream, item: TokenStream) -> TokenStream {
    commands_impl(attr, item)
}

/// Alias — `#[system]` is identical to `#[commands]` (handles both `#[command]` and `#[notice]`/`#[request]`/`#[meta]`).
#[proc_macro_attribute]
pub fn system(attr: TokenStream, item: TokenStream) -> TokenStream {
    commands_impl(attr, item)
}

// ─── System event routing types ─────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum SystemEventKind {
    Notice,
    Request,
    Meta,
}

struct SysMethod {
    method_name: Ident,
    event_kind: SystemEventKind,
    route_variants: Vec<Ident>,
    extra_params: usize,
    first_param_is_ctx: bool,
}

/// Parse `#[notice(GroupPoke, PrivatePoke)]` / `#[request(...)]` / `#[meta(...)]`
fn parse_system_attr(attr: &syn::Attribute, kind: SystemEventKind) -> syn::Result<(SystemEventKind, Vec<Ident>)> {
    let variants: Punctuated<Ident, Token![,]> = attr.parse_args_with(
        Punctuated::<Ident, Token![,]>::parse_terminated,
    )?;
    if variants.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "system event attribute requires at least one route variant",
        ));
    }
    Ok((kind, variants.into_iter().collect()))
}

fn commands_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as CommandsMacroArgs);
    let input = parse_macro_input!(item as ItemImpl);

    // Infer module name from impl block's Self type if not explicitly provided
    let module_name: Ident = if let Some(name) = args.module_name {
        name
    } else {
        match input.self_ty.as_ref() {
            syn::Type::Path(tp) => tp
                .path
                .get_ident()
                .cloned()
                .unwrap_or_else(|| Ident::new("Unknown", Span::call_site())),
            _ => {
                return syn::Error::new_spanned(
                    &input.self_ty,
                    "#[commands]: cannot infer module name from complex self type; \
                     use #[commands(ModuleName)]",
                )
                .to_compile_error()
                .into();
            }
        }
    };
    let self_ty = &input.self_ty;

    // Collect command methods and system methods, strip macro attributes
    struct CmdMethod {
        method_name: Ident,
        args: CommandArgs,
        extra_params: usize,
        first_param_is_ctx: bool,
    }

    let mut commands = Vec::new();
    let mut system_methods: Vec<SysMethod> = Vec::new();
    let mut clean_items = Vec::new();

    for item in &input.items {
        match item {
            syn::ImplItem::Fn(method) => {
                let mut cmd_attr = None;
                let mut sys_attr = None;
                let mut other_attrs = Vec::new();

                let method_name_str = method.sig.ident.to_string();
                for attr in &method.attrs {
                    if attr.path().is_ident("command") {
                        match parse_command_attr(attr, &method_name_str) {
                            Ok(parsed) => cmd_attr = Some(parsed),
                            Err(e) => return e.to_compile_error().into(),
                        }
                    } else if attr.path().is_ident("notice") {
                        match parse_system_attr(attr, SystemEventKind::Notice) {
                            Ok(parsed) => sys_attr = Some(parsed),
                            Err(e) => return e.to_compile_error().into(),
                        }
                    } else if attr.path().is_ident("request") {
                        match parse_system_attr(attr, SystemEventKind::Request) {
                            Ok(parsed) => sys_attr = Some(parsed),
                            Err(e) => return e.to_compile_error().into(),
                        }
                    } else if attr.path().is_ident("meta") {
                        match parse_system_attr(attr, SystemEventKind::Meta) {
                            Ok(parsed) => sys_attr = Some(parsed),
                            Err(e) => return e.to_compile_error().into(),
                        }
                    } else {
                        other_attrs.push(attr.clone());
                    }
                }

                // Collect typed params (everything after &self)
                let params: Vec<_> = method
                    .sig
                    .inputs
                    .iter()
                    .filter(|arg| matches!(arg, syn::FnArg::Typed(_)))
                    .collect();
                let extra_params = params.len();
                let first_param_is_ctx = params.first().map_or(false, |arg| {
                    let tokens = quote! { #arg }.to_string();
                    tokens.contains("PluginContext")
                });

                if let Some(cmd_args) = cmd_attr {
                    commands.push(CmdMethod {
                        method_name: method.sig.ident.clone(),
                        args: cmd_args,
                        extra_params,
                        first_param_is_ctx,
                    });
                }

                if let Some((kind, variants)) = sys_attr {
                    system_methods.push(SysMethod {
                        method_name: method.sig.ident.clone(),
                        event_kind: kind,
                        route_variants: variants,
                        extra_params,
                        first_param_is_ctx,
                    });
                }

                // Rebuild method without macro attrs
                let mut clean_method = method.clone();
                clean_method.attrs = other_attrs;
                clean_items.push(syn::ImplItem::Fn(clean_method));
            }
            other => clean_items.push(other.clone()),
        }
    }

    // Build the cleaned impl block — strip #[commands]/#[system] from impl attrs
    let impl_attrs: Vec<_> = input
        .attrs
        .iter()
        .filter(|a| {
            !a.path().is_ident("commands")
                && !a.path().is_ident("system")
                && !a.path().is_ident("qimen_commands")
        })
        .collect();
    let impl_generics = &input.generics;
    let clean_impl = quote! {
        #(#impl_attrs)*
        impl #impl_generics #self_ty {
            #(#clean_items)*
        }
    };

    let has_commands = !commands.is_empty();
    let has_system = !system_methods.is_empty();

    // ─── Command plugin generation ──────────────────────────────────────

    let cmd_plugin_ident = format_ident!("__QimenCmdPlugin_{}", module_name);

    let cmd_plugin_tokens = if has_commands {
        let cmd_def_exprs: Vec<_> = commands
            .iter()
            .map(|cmd| {
                let name = &cmd.args.name;
                let desc = &cmd.args.desc;
                let aliases = &cmd.args.aliases;
                let examples = &cmd.args.examples;
                let category = cmd.args.category.as_deref().unwrap_or("general");
                let hidden = cmd.args.hidden;

                let role_expr = match cmd.args.role.as_deref() {
                    Some("admin" | "Admin") => {
                        quote! { .role(qimen_plugin_api::CommandRole::Admin) }
                    }
                    Some("owner" | "Owner") => {
                        quote! { .role(qimen_plugin_api::CommandRole::Owner) }
                    }
                    _ => quote! {},
                };

                let hidden_expr = if hidden {
                    quote! { .hidden() }
                } else {
                    quote! {}
                };

                quote! {
                    qimen_plugin_api::CommandDefinition::new(#name, #desc)
                        .aliases(&[#(#aliases),*])
                        .examples(&[#(#examples),*])
                        .category(#category)
                        #hidden_expr
                        #role_expr
                }
            })
            .collect();

        let match_arms: Vec<_> = commands
            .iter()
            .map(|cmd| {
                let name = &cmd.args.name;
                let method_name = &cmd.method_name;
                let call_expr = match cmd.extra_params {
                    0 => quote! { qimen_plugin_api::IntoCommandSignal::into_signal(__inst.#method_name().await) },
                    1 => {
                        if cmd.first_param_is_ctx {
                            quote! { qimen_plugin_api::IntoCommandSignal::into_signal(__inst.#method_name(__ctx).await) }
                        } else {
                            quote! { qimen_plugin_api::IntoCommandSignal::into_signal(__inst.#method_name(__invocation.args.clone()).await) }
                        }
                    }
                    _ => {
                        quote! { qimen_plugin_api::IntoCommandSignal::into_signal(__inst.#method_name(__ctx, __invocation.args.clone()).await) }
                    }
                };
                quote! {
                    #name => {
                        let __inst = #module_name;
                        Some(#call_expr)
                    }
                }
            })
            .collect();

        quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            struct #cmd_plugin_ident;

            #[async_trait::async_trait]
            impl qimen_plugin_api::CommandPlugin for #cmd_plugin_ident {
                fn metadata(&self) -> qimen_plugin_api::PluginMetadata {
                    qimen_plugin_api::PluginMetadata {
                        id: #module_name::__QIMEN_MODULE_ID,
                        name: #module_name::__QIMEN_MODULE_NAME,
                        version: #module_name::__QIMEN_MODULE_VERSION,
                        description: #module_name::__QIMEN_MODULE_DESCRIPTION,
                        api_version: "0.1",
                        compatibility: qimen_plugin_api::PluginCompatibility {
                            host_api: "0.1",
                            framework_min: "0.1.0",
                            framework_max: "0.1.x",
                        },
                    }
                }

                fn commands(&self) -> Vec<qimen_plugin_api::CommandDefinition> {
                    vec![
                        #(#cmd_def_exprs),*
                    ]
                }

                async fn on_command(
                    &self,
                    __ctx: &qimen_plugin_api::CommandPluginContext<'_>,
                    __invocation: &qimen_plugin_api::CommandInvocation,
                ) -> Option<qimen_plugin_api::CommandPluginSignal> {
                    match __invocation.definition.name {
                        #(#match_arms)*
                        _ => Some(qimen_plugin_api::CommandPluginSignal::Continue),
                    }
                }
            }
        }
    } else {
        quote! {}
    };

    // ─── System plugin generation ───────────────────────────────────────

    let sys_plugin_ident = format_ident!("__QimenSysPlugin_{}", module_name);

    let sys_plugin_tokens = if has_system {
        // Group system methods by event kind
        let notice_methods: Vec<_> = system_methods
            .iter()
            .filter(|m| m.event_kind == SystemEventKind::Notice)
            .collect();
        let request_methods: Vec<_> = system_methods
            .iter()
            .filter(|m| m.event_kind == SystemEventKind::Request)
            .collect();
        let meta_methods: Vec<_> = system_methods
            .iter()
            .filter(|m| m.event_kind == SystemEventKind::Meta)
            .collect();

        fn gen_sys_match_arms(
            methods: &[&SysMethod],
            route_enum: &proc_macro2::TokenStream,
            module_name: &Ident,
        ) -> Vec<proc_macro2::TokenStream> {
            let mut arms = Vec::new();
            for m in methods {
                let method_name = &m.method_name;
                let call_expr = match m.extra_params {
                    0 => quote! {
                        qimen_plugin_api::IntoSystemSignal::into_signal(
                            __inst.#method_name().await
                        )
                    },
                    1 => {
                        if m.first_param_is_ctx {
                            quote! {
                                qimen_plugin_api::IntoSystemSignal::into_signal(
                                    __inst.#method_name(__ctx).await
                                )
                            }
                        } else {
                            quote! {
                                qimen_plugin_api::IntoSystemSignal::into_signal(
                                    __inst.#method_name(__route).await
                                )
                            }
                        }
                    }
                    _ => quote! {
                        qimen_plugin_api::IntoSystemSignal::into_signal(
                            __inst.#method_name(__ctx, __route).await
                        )
                    },
                };

                for variant in &m.route_variants {
                    let call = call_expr.clone();
                    arms.push(quote! {
                        #route_enum::#variant => {
                            let __inst = #module_name;
                            Some(#call)
                        }
                    });
                }
            }
            arms
        }

        let on_notice_impl = if notice_methods.is_empty() {
            quote! {}
        } else {
            let route_enum = quote! { qimen_plugin_api::SystemNoticeRoute };
            let arms = gen_sys_match_arms(&notice_methods, &route_enum, &module_name);
            quote! {
                async fn on_notice(
                    &self,
                    __ctx: &qimen_plugin_api::SystemPluginContext<'_>,
                    __route: &qimen_plugin_api::SystemNoticeRoute,
                ) -> Option<qimen_plugin_api::SystemPluginSignal> {
                    match __route {
                        #(#arms)*
                        _ => None,
                    }
                }
            }
        };

        let on_request_impl = if request_methods.is_empty() {
            quote! {}
        } else {
            let route_enum = quote! { qimen_plugin_api::SystemRequestRoute };
            let arms = gen_sys_match_arms(&request_methods, &route_enum, &module_name);
            quote! {
                async fn on_request(
                    &self,
                    __ctx: &qimen_plugin_api::SystemPluginContext<'_>,
                    __route: &qimen_plugin_api::SystemRequestRoute,
                ) -> Option<qimen_plugin_api::SystemPluginSignal> {
                    match __route {
                        #(#arms)*
                        _ => None,
                    }
                }
            }
        };

        let on_meta_impl = if meta_methods.is_empty() {
            quote! {}
        } else {
            let route_enum = quote! { qimen_plugin_api::SystemMetaRoute };
            let arms = gen_sys_match_arms(&meta_methods, &route_enum, &module_name);
            quote! {
                async fn on_meta(
                    &self,
                    __ctx: &qimen_plugin_api::SystemPluginContext<'_>,
                    __route: &qimen_plugin_api::SystemMetaRoute,
                ) -> Option<qimen_plugin_api::SystemPluginSignal> {
                    match __route {
                        #(#arms)*
                        _ => None,
                    }
                }
            }
        };

        quote! {
            #[doc(hidden)]
            #[allow(non_camel_case_types)]
            struct #sys_plugin_ident;

            #[async_trait::async_trait]
            impl qimen_plugin_api::SystemPlugin for #sys_plugin_ident {
                fn metadata(&self) -> qimen_plugin_api::PluginMetadata {
                    qimen_plugin_api::PluginMetadata {
                        id: #module_name::__QIMEN_MODULE_ID,
                        name: #module_name::__QIMEN_MODULE_NAME,
                        version: #module_name::__QIMEN_MODULE_VERSION,
                        description: #module_name::__QIMEN_MODULE_DESCRIPTION,
                        api_version: "0.1",
                        compatibility: qimen_plugin_api::PluginCompatibility {
                            host_api: "0.1",
                            framework_min: "0.1.0",
                            framework_max: "0.1.x",
                        },
                    }
                }

                #on_notice_impl
                #on_request_impl
                #on_meta_impl
            }
        }
    } else {
        quote! {}
    };

    // ─── Module impl ────────────────────────────────────────────────────

    let command_plugins_body = if has_commands {
        quote! { vec![std::sync::Arc::new(#cmd_plugin_ident)] }
    } else {
        quote! { vec![] }
    };

    let system_plugins_body = if has_system {
        quote! {
            let mut __sys = #module_name::__qimen_system_plugins();
            __sys.push(std::sync::Arc::new(#sys_plugin_ident) as std::sync::Arc<dyn qimen_plugin_api::SystemPlugin>);
            __sys
        }
    } else {
        quote! { #module_name::__qimen_system_plugins() }
    };

    let expanded = quote! {
        #clean_impl
        #cmd_plugin_tokens
        #sys_plugin_tokens

        #[async_trait::async_trait]
        impl qimen_plugin_api::Module for #module_name {
            fn id(&self) -> &'static str {
                #module_name::__QIMEN_MODULE_ID
            }

            async fn on_load(&self) -> qimen_error::Result<()> {
                Ok(())
            }

            fn command_plugins(&self) -> Vec<std::sync::Arc<dyn qimen_plugin_api::CommandPlugin>> {
                #command_plugins_body
            }

            fn system_plugins(&self) -> Vec<std::sync::Arc<dyn qimen_plugin_api::SystemPlugin>> {
                #system_plugins_body
            }

            fn interceptors(&self) -> Vec<std::sync::Arc<dyn qimen_plugin_api::MessageEventInterceptor>> {
                #module_name::__qimen_interceptors()
            }
        }
    };

    expanded.into()
}
