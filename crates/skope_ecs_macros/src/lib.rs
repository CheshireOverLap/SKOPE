use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for `Component` marker trait.
#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    TokenStream::from(quote! {
        impl #impl_generics skope_ecs::Component for #name #ty_generics #where_clause {}
    })
}

/// Derive macro for `Resource` marker trait.
#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    TokenStream::from(quote! {
        impl #impl_generics skope_ecs::Resource for #name #ty_generics #where_clause {}
    })
}

/// Derive macro for `Event` marker trait.
#[proc_macro_derive(Event)]
pub fn derive_event(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    TokenStream::from(quote! {
        impl #impl_generics skope_ecs::Event for #name #ty_generics #where_clause {}
    })
}

/// Derive macro for `SystemSet` marker trait.
/// Requires the type to also derive `Debug + Clone + PartialEq + Eq + Hash`.
#[proc_macro_derive(SystemSet)]
pub fn derive_system_set(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    TokenStream::from(quote! {
        impl #impl_generics skope_ecs::SystemSet for #name #ty_generics #where_clause {}
    })
}
