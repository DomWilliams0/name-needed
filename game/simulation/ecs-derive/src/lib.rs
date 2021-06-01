use proc_macro::TokenStream;
use proc_macro_error::*;
use quote::quote;
use syn::spanned::Spanned;
use syn::*;

use proc_macro_error::proc_macro_error;
use syn::DeriveInput;

/// Expects `simulation::ecs::*` to be imported
#[proc_macro_derive(EcsComponent, attributes(name, interactive))]
#[proc_macro_error]
pub fn ecs_component_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = extract_name(&input);
    let interactive = extract_interactive(&input);
    let comp = input.ident;

    let as_interactive = if interactive {
        quote! {
            InteractiveResult::Interactive(comp as &dyn InteractiveComponent)
        }
    } else {
        quote! {
            InteractiveResult::NonInteractive
        }
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

            fn get_interactive(world: &EcsWorld, entity: Entity) -> Option<InteractiveResult> {
                world.component::<Self>(entity)
                .ok()
                .map(|comp| #as_interactive)
            }
        }

        inventory::submit!(ComponentEntry {
            name: #name,
            has_comp_fn: #comp ::has_component,
            register_comp_fn: #comp ::register_component,
            get_interactive_fn: #comp ::get_interactive,
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
