use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use source_analyzer::{struct_hierarchy::StructHierarchy, structs::Structs};

pub fn generate_structs(structs: &Structs) -> TokenStream {
    let derives = quote! {
        #[derive(
            Clone,
            Debug,
            Default,
            serde::Deserialize,
            serde::Serialize,
            serialize_hierarchy::SerializeHierarchy,
         )]
    };
    let configuration = hierarchy_to_token_stream(
        &structs.configuration,
        format_ident!("Configuration"),
        &derives,
    );
    let cyclers = structs
        .cyclers
        .iter()
        .map(|(cycler_module, cycler_structs)| {
            let main_outputs = hierarchy_to_token_stream(
                &cycler_structs.main_outputs,
                format_ident!("MainOutputs"),
                &derives,
            );
            let additional_outputs = hierarchy_to_token_stream(
                &cycler_structs.additional_outputs,
                format_ident!("AdditionalOutputs"),
                &derives,
            );
            let persistent_state = hierarchy_to_token_stream(
                &cycler_structs.persistent_state,
                format_ident!("PersistentState"),
                &derives,
            );

            quote! {
                #[allow(non_snake_case, non_camel_case_types)]
                pub mod #cycler_module {
                    #main_outputs
                    #additional_outputs
                    #persistent_state
                }
            }
        });

    quote! {
        #[allow(non_snake_case, non_camel_case_types)]
        #configuration
        #(#cyclers)*
    }
}

fn hierarchy_to_token_stream(
    hierarchy: &StructHierarchy,
    struct_name: Ident,
    derives: &TokenStream,
) -> TokenStream {
    let fields = match hierarchy {
        StructHierarchy::Struct { fields } => fields,
        StructHierarchy::Optional { .. } => panic!("option instead of struct"),
        StructHierarchy::Field { .. } => panic!("field instead of struct"),
    };
    let struct_fields = fields
        .iter()
        .map(|(name, struct_hierarchy)| match struct_hierarchy {
            StructHierarchy::Struct { .. } => {
                let struct_name_identifier = format_ident!("{}_{}", struct_name, name);
                quote! { pub #name: #struct_name_identifier }
            }
            StructHierarchy::Optional { child } => match &**child {
                StructHierarchy::Struct { .. } => {
                    let struct_name_identifier = format_ident!("{}_{}", struct_name, name);
                    quote! { pub #name: Option<#struct_name_identifier> }
                }
                StructHierarchy::Optional { .. } => {
                    panic!("unexpected optional in an optional struct")
                }
                StructHierarchy::Field { data_type } => {
                    quote! { pub #name: Option<#data_type> }
                }
            },
            StructHierarchy::Field { data_type } => {
                quote! { pub #name: #data_type }
            }
        });
    let child_structs = fields.iter().map(|(name, struct_hierarchy)| {
        let struct_name = format_ident!("{}_{}", struct_name, name);
        match struct_hierarchy {
            StructHierarchy::Struct { .. } => {
                hierarchy_to_token_stream(struct_hierarchy, struct_name, derives)
            }
            StructHierarchy::Optional { child } => match &**child {
                StructHierarchy::Struct { .. } => {
                    hierarchy_to_token_stream(struct_hierarchy, struct_name, derives)
                }
                StructHierarchy::Optional { .. } => {
                    panic!("unexpected optional in an optional struct")
                }
                StructHierarchy::Field { .. } => quote! {},
            },
            StructHierarchy::Field { .. } => quote! {},
        }
    });
    quote! {
        #derives
        pub struct #struct_name {
            #(#struct_fields,)*
        }
        #(#child_structs)*
    }
}
