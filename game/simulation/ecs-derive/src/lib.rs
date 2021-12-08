use proc_macro::TokenStream;
use proc_macro_error::*;
use quote::quote;
use syn::spanned::Spanned;
use syn::*;

use proc_macro_error::proc_macro_error;
use syn::DeriveInput;

enum CloneBehaviour {
    Allow,
    Disallow,
}

/// Expects `simulation::ecs::*` to be imported
#[proc_macro_derive(EcsComponent, attributes(name, interactive, clone))]
#[proc_macro_error]
pub fn ecs_component_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = extract_name(&input);
    let interactive = extract_interactive(&input);
    let clone = extract_clone_behaviour(&input);
    let comp = input.ident;

    let as_interactive = if interactive {
        quote! {
            let actually_self = &*(actually_self as *const () as *const Self);
            Some(actually_self as &dyn InteractiveComponent)
        }
    } else {
        quote! {
            None
        }
    };

    let (clone, clone_fn) = match clone {
        CloneBehaviour::Allow => (
            quote! {
                let mut storage = world.write_storage::<Self>();
                let comp = storage.get(source.into()).cloned();
                if let Some(comp) = comp {
                    // assume entity is alive
                    let _ = storage.insert(dest.into(), comp);
                }
            },
            quote! { Some(#comp ::clone_to), },
        ),
        CloneBehaviour::Disallow => (quote! {}, quote! { None }),
    };

    let result = quote! {
        impl #comp {
            pub const COMPONENT_NAME: &'static str = #name;

            fn has_component(world: &EcsWorld, entity: Entity) -> bool {
                world.has_component::<Self>(entity)
            }

            fn register_component(world: &mut SpecsWorld) {
                world.register::<Self>();
            }

            fn get_component(world: &EcsWorld, entity: Entity) -> Option<ComponentRefErased> {
                world.component::<Self>(entity).ok().map(|comp_ref| comp_ref.erased(Self::as_interactive))
            }

            unsafe fn as_interactive(actually_self: &()) -> Option<&dyn InteractiveComponent> {
                #as_interactive
            }

            fn clone_to(world: &EcsWorld, source: Entity, dest: Entity) {
                #clone
            }
        }

        inventory::submit!(ComponentEntry {
            name: #name,
            has_comp_fn: #comp ::has_component,
            register_comp_fn: #comp ::register_component,
            get_comp_fn: #comp ::get_component,
            clone_to_fn: #clone_fn
        });
    };

    TokenStream::from(result)
}

fn extract_name(item: &DeriveInput) -> String {
    let span = item.span();
    let attribute = item
        .attrs
        .iter()
        .find(|a| a.path.is_ident("name"))
        .unwrap_or_else(|| abort!(span, "expected name attribute for {}", item.ident));

    let name: syn::LitStr = attribute
        .parse_args()
        .expect("name must be a string literal");
    name.value()
}

fn extract_interactive(item: &DeriveInput) -> bool {
    item.attrs.iter().any(|a| a.path.is_ident("interactive"))
}

fn extract_clone_behaviour(item: &DeriveInput) -> CloneBehaviour {
    let span = item.span();
    let attribute = item.attrs.iter().find(|a| a.path.is_ident("clone"));

    attribute
        .and_then(|a| a.parse_args::<syn::Ident>().ok())
        .map(|s| {
            if s == "disallow" {
                CloneBehaviour::Disallow
            } else {
                abort!(span, "invalid clone attribute for {}", item.ident)
            }
        })
        .unwrap_or(CloneBehaviour::Allow)
}
