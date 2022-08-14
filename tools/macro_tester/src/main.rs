use std::{collections::BTreeMap, fs::File, io::Write, path::Path, process::Command};

use anyhow::{anyhow, bail, Context};
use convert_case::{Case, Casing};
use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, ToTokens};
use source_analyzer::{CyclerInstances, CyclerType, CyclerTypes, Field, Modules};

pub fn write_token_stream<P>(file_path: P, token_stream: TokenStream) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    {
        let mut file = File::create(&file_path)
            .with_context(|| anyhow!("Failed create file {:?}", file_path.as_ref()))?;
        write!(file, "{}", token_stream)
            .with_context(|| anyhow!("Failed to write to file {:?}", file_path.as_ref()))?;
    }

    let status = Command::new("rustfmt")
        .arg(file_path.as_ref())
        .status()
        .context("Failed to execute rustfmt")?;
    if !status.success() {
        bail!("rustfmt did not exit with success");
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cycler_instances = CyclerInstances::try_from_crates_directory("crates")
        .context("Failed to get cycler instances from crates directory")?;
    let mut modules = Modules::try_from_crates_directory("crates")
        .context("Failed to get modules from crates directory")?;
    modules.sort().context("Failed to sort modules")?;
    let cycler_types = CyclerTypes::try_from_crates_directory("crates")
        .context("Failed to get perception cycler instances from crates directory")?;

    let cyclers = generate_cyclers(&cycler_instances, &modules, &cycler_types)
        .context("Failed to generate cyclers")?;

    write_token_stream("cyclers.rs", cyclers).context("Failed to write cyclers")?;

    println!("cycler_instances: {cycler_instances:#?}");
    println!("modules: {modules:#?}");
    println!("cycler_types: {cycler_types:#?}");

    Ok(())
}

fn generate_cyclers(
    cycler_instances: &CyclerInstances,
    modules: &Modules,
    cycler_types: &CyclerTypes,
) -> anyhow::Result<TokenStream> {
    let cyclers: Vec<_> = cycler_instances
        .modules_to_instances
        .keys()
        .map(|cycler_module_name| {
            let cycler = match cycler_types.cycler_modules_to_cycler_types[cycler_module_name] {
                CyclerType::Perception => Cycler::Perception {
                    cycler_instances,
                    modules,
                    cycler_types,
                    cycler_module_name,
                },
                CyclerType::RealTime => Cycler::RealTime {
                    cycler_instances,
                    modules,
                    cycler_types,
                    cycler_module_name,
                },
            };

            cycler
                .get_module()
                .with_context(|| anyhow!("Failed to get cycler `{cycler_module_name}`"))
        })
        .collect::<Result<_, _>>()
        .context("Failed to get cyclers")?;

    Ok(quote! {
        #[derive(Default)]
        pub struct Outputs<MainOutputs, AdditionalOutputs>
        where
            MainOutputs: Default,
            AdditionalOutputs: Default,
        {
            pub main_outputs: MainOutputs,
            pub additional_outputs: AdditionalOutputs,
        }

        #(#cyclers)*
    })
}

enum Cycler<'a> {
    Perception {
        cycler_instances: &'a CyclerInstances,
        modules: &'a Modules,
        cycler_types: &'a CyclerTypes,
        cycler_module_name: &'a str,
    },
    RealTime {
        cycler_instances: &'a CyclerInstances,
        modules: &'a Modules,
        cycler_types: &'a CyclerTypes,
        cycler_module_name: &'a str,
    },
}

impl Cycler<'_> {
    fn get_cycler_instances(&self) -> &CyclerInstances {
        match self {
            Cycler::Perception {
                cycler_instances, ..
            } => cycler_instances,
            Cycler::RealTime {
                cycler_instances, ..
            } => cycler_instances,
        }
    }

    fn get_modules(&self) -> &Modules {
        match self {
            Cycler::Perception { modules, .. } => modules,
            Cycler::RealTime { modules, .. } => modules,
        }
    }

    fn get_cycler_types(&self) -> &CyclerTypes {
        match self {
            Cycler::Perception { cycler_types, .. } => cycler_types,
            Cycler::RealTime { cycler_types, .. } => cycler_types,
        }
    }

    fn get_cycler_module_name(&self) -> &str {
        match self {
            Cycler::Perception {
                cycler_module_name, ..
            } => cycler_module_name,
            Cycler::RealTime {
                cycler_module_name, ..
            } => cycler_module_name,
        }
    }

    fn get_cycler_module_name_identifier(&self) -> Ident {
        format_ident!("{}", self.get_cycler_module_name())
    }

    fn get_own_writer_type(&self) -> TokenStream {
        let cycler_module_name_identifier = self.get_cycler_module_name_identifier();
        quote! {
            framework::Writer<
                crate::Outputs<
                    structs::#cycler_module_name_identifier::MainOutputs,
                    structs::#cycler_module_name_identifier::AdditionalOutputs,
                >
            >
        }
    }

    fn get_own_producer_identifier(&self) -> TokenStream {
        let own_producer_type = self.get_own_producer_type();
        match self {
            Cycler::Perception { .. } => quote! { own_producer, },
            Cycler::RealTime { .. } => Default::default(),
        }
    }

    fn get_own_producer_type(&self) -> TokenStream {
        let cycler_module_name_identifier = self.get_cycler_module_name_identifier();
        quote! {
            framework::Producer<
                structs::#cycler_module_name_identifier::MainOutputs,
            >
        }
    }

    fn get_own_producer_field(&self) -> TokenStream {
        let own_producer_type = self.get_own_producer_type();
        match self {
            Cycler::Perception { .. } => quote! { own_producer: #own_producer_type, },
            Cycler::RealTime { .. } => Default::default(),
        }
    }

    fn get_other_cyclers(&self) -> Vec<OtherCycler> {
        match self {
            Cycler::Perception {
                cycler_instances,
                cycler_types,
                ..
            } => cycler_types
                .cycler_modules_to_cycler_types
                .iter()
                .filter_map(
                    |(other_cycler_module_name, other_cycler_type)| match other_cycler_type {
                        CyclerType::RealTime => Some(
                            cycler_instances.modules_to_instances[other_cycler_module_name]
                                .iter()
                                .map(|other_cycler_instance_name| OtherCycler::Reader {
                                    cycler_instance_name: other_cycler_instance_name,
                                    cycler_module_name: other_cycler_module_name,
                                }),
                        ),
                        CyclerType::Perception => None,
                    },
                )
                .flatten()
                .collect(),
            Cycler::RealTime {
                cycler_instances,
                cycler_types,
                ..
            } => cycler_types
                .cycler_modules_to_cycler_types
                .iter()
                .filter_map(
                    |(other_cycler_module_name, other_cycler_type)| match other_cycler_type {
                        CyclerType::Perception => Some(
                            cycler_instances.modules_to_instances[other_cycler_module_name]
                                .iter()
                                .map(|other_cycler_instance_name| OtherCycler::Consumer {
                                    cycler_instance_name: other_cycler_instance_name,
                                    cycler_module_name: other_cycler_module_name,
                                }),
                        ),
                        CyclerType::RealTime => None,
                    },
                )
                .flatten()
                .collect(),
        }
    }

    fn get_other_cycler_identifiers(&self) -> Vec<Ident> {
        self.get_other_cyclers()
            .into_iter()
            .map(|other_cycler| match other_cycler {
                OtherCycler::Consumer {
                    cycler_instance_name,
                    ..
                } => format_ident!("{}_consumer", cycler_instance_name.to_case(Case::Snake)),
                OtherCycler::Reader {
                    cycler_instance_name,
                    ..
                } => format_ident!("{}_reader", cycler_instance_name.to_case(Case::Snake)),
            })
            .collect()
    }

    fn get_other_cycler_fields(&self) -> Vec<TokenStream> {
        self.get_other_cyclers()
            .into_iter()
            .map(|other_cycler| {
                let (field_name, field_type) = match other_cycler {
                    OtherCycler::Consumer {
                        cycler_instance_name,
                        cycler_module_name,
                    } => {
                        let cycler_module_name_identifier = format_ident!("{}", cycler_module_name);
                        (
                            format_ident!("{}_consumer", cycler_instance_name.to_case(Case::Snake)),
                            quote! {
                                framework::Consumer<
                                    structs::#cycler_module_name_identifier::MainOutputs,
                                >
                            },
                        )
                    }
                    OtherCycler::Reader {
                        cycler_instance_name,
                        cycler_module_name,
                    } => {
                        let cycler_module_name_identifier = format_ident!("{}", cycler_module_name);
                        (
                            format_ident!("{}_reader", cycler_instance_name.to_case(Case::Snake)),
                            quote! {
                                framework::Reader<
                                    structs::#cycler_module_name_identifier::MainOutputs,
                                >
                            },
                        )
                    }
                };
                quote! {
                    #field_name: #field_type
                }
            })
            .collect()
    }

    fn get_interpreted_modules(&self) -> Vec<Module> {
        self.get_modules()
            .modules
            .iter()
            .filter_map(|(module_name, module)| {
                if module.cycler_module != self.get_cycler_module_name() {
                    return None;
                }

                match self {
                    Cycler::Perception { .. } => Some(Module::Perception {
                        module_name,
                        module,
                    }),
                    Cycler::RealTime { .. } => Some(Module::RealTime {
                        module_name,
                        module,
                    }),
                }
            })
            .collect()
    }

    fn get_module_identifiers(&self) -> Vec<Ident> {
        self.get_interpreted_modules()
            .into_iter()
            .map(|module| module.get_identifier_snake_case())
            .collect()
    }

    fn get_module_fields(&self) -> Vec<TokenStream> {
        self.get_interpreted_modules()
            .into_iter()
            .map(|module| module.get_field())
            .collect()
    }

    fn get_module_initializers(&self) -> anyhow::Result<Vec<TokenStream>> {
        self.get_interpreted_modules()
            .into_iter()
            .map(|module| module.get_initializer())
            .collect()
    }

    fn get_module_executions(&self) -> anyhow::Result<Vec<TokenStream>> {
        self.get_interpreted_modules()
            .into_iter()
            .map(|module| module.get_execution())
            .collect()
    }

    fn get_struct_definition(&self) -> TokenStream {
        let own_writer_type = self.get_own_writer_type();
        let own_producer_field = self.get_own_producer_field();
        let other_cycler_fields = self.get_other_cycler_fields();
        let cycler_module_name_identifier = self.get_cycler_module_name_identifier();
        let module_fields = self.get_module_fields();

        quote! {
            pub struct Cycler<Instance> {
                instance_name: String,
                hardware_interface: std::sync::Arc<Interface>,
                own_writer: #own_writer_type,
                #own_producer_field
                #(#other_cycler_fields,)*
                configuration_reader: framework::Reader<structs::Configuration>,
                persistent_state: structs::#cycler_module_name_identifier::PersistentState,
                #(#module_fields,)*
            }
        }
    }

    fn get_new_method(&self) -> anyhow::Result<TokenStream> {
        let own_writer_type = self.get_own_writer_type();
        let own_producer_field = self.get_own_producer_field();
        let other_cycler_fields = self.get_other_cycler_fields();
        let cycler_module_name_identifier = self.get_cycler_module_name_identifier();
        let module_initializers = self
            .get_module_initializers()
            .context("Failed to get module initializers")?;
        let own_producer_identifier = self.get_own_producer_identifier();
        let other_cycler_identifiers = self.get_other_cycler_identifiers();
        let module_identifiers = self.get_module_identifiers();

        Ok(quote! {
            pub fn new(
                instance_name: String,
                hardware_interface: std::sync::Arc<Interface>,
                own_writer: #own_writer_type,
                #own_producer_field
                #(#other_cycler_fields,)*
                configuration_reader: framework::Reader<structs::Configuration>,
                persistent_state: structs::#cycler_module_name_identifier::PersistentState,
            ) -> anyhow::Result<Self> {
                use anyhow::Context;
                let configuration = configuration_reader.next().clone();
                let mut persistent_state = Default::default();
                #(#module_initializers)*
                Ok(Self {
                    instance_name,
                    hardware_interface,
                    own_writer,
                    #own_producer_identifier
                    #(#other_cycler_identifiers,)*
                    configuration_reader,
                    persistent_state,
                    #(#module_identifiers,)*
                })
            }
        })
    }

    fn get_start_method(&self) -> TokenStream {
        quote! {
            pub fn start(
                mut self,
                keep_running: tokio_util::sync::CancellationToken,
            ) -> anyhow::Result<std::thread::JoinHandle<anyhow::Result<()>>> {
                use anyhow::Context;
                let instance_name = self.instance_name.clone();
                std::thread::Builder::new()
                    .name(instance_name.clone())
                    .spawn(move || {
                        while !keep_running.is_cancelled() {
                            if let Err(error) = self.cycle() {
                                keep_running.cancel();
                                return Err(error).context("Failed to execute cycle of cycler");
                            }
                        }
                        Ok(())
                    })
                    .with_context(|| {
                        anyhow::anyhow!("Failed to spawn thread for `{instance_name}`")
                    })
            }
        }
    }

    fn get_cycle_method(&self) -> anyhow::Result<TokenStream> {
        let module_executions = self
            .get_module_executions()
            .context("Failed to get module executions")?;

        if module_executions.is_empty() {
            bail!("Expected at least one module");
        }

        let (first_module, remaining_modules) = module_executions.split_at(1);
        let first_module = &first_module[0];
        let remaining_module_executions = match remaining_modules.is_empty() {
            true => Default::default(),
            false => quote! {
                {
                    let configuration = self.configuration_reader.next();

                    #(#remaining_modules)*
                }
            },
        };

        Ok(quote! {
            fn cycle(&mut self) -> anyhow::Result<()> {
                use anyhow::Context;

                {
                    let mut own_database = self.own_writer.next();

                    {
                        let configuration = self.configuration_reader.next();

                        #first_module
                    }

                    self.own_producer.announce();

                    #remaining_module_executions

                    self.own_producer.finalize(own_database.main_outputs.clone());
                }
            }
        })
    }

    fn get_struct_implementation(&self) -> anyhow::Result<TokenStream> {
        let new_method = self
            .get_new_method()
            .context("Failed to get `new` method")?;
        let start_method = self.get_start_method();
        let cycle_method = self
            .get_cycle_method()
            .context("Failed to get `cycle` method")?;

        Ok(quote! {
            impl<Interface> Cycler<Interface>
            where
                Interface: hardware::HardwareInterface + Send + Sync + 'static,
            {
                #new_method
                #start_method
                #cycle_method
            }
        })
    }

    fn get_module(&self) -> anyhow::Result<TokenStream> {
        let cycler_module_name_identifier = self.get_cycler_module_name_identifier();
        let struct_definition = self.get_struct_definition();
        let struct_implementation = self
            .get_struct_implementation()
            .context("Failed to get struct implementation")?;

        Ok(quote! {
            pub mod #cycler_module_name_identifier {
                #struct_definition
                #struct_implementation
            }
        })
    }
}

enum OtherCycler<'a> {
    Consumer {
        cycler_instance_name: &'a str,
        cycler_module_name: &'a str,
    },
    Reader {
        cycler_instance_name: &'a str,
        cycler_module_name: &'a str,
    },
}

enum Module<'a> {
    Perception {
        module_name: &'a str,
        module: &'a source_analyzer::Module,
    },
    RealTime {
        module_name: &'a str,
        module: &'a source_analyzer::Module,
    },
}

impl Module<'_> {
    fn get_module_name(&self) -> &str {
        match self {
            Module::Perception { module_name, .. } => module_name,
            Module::RealTime { module_name, .. } => module_name,
        }
    }

    fn get_module(&self) -> &source_analyzer::Module {
        match self {
            Module::Perception { module, .. } => module,
            Module::RealTime { module, .. } => module,
        }
    }

    fn get_identifier(&self) -> Ident {
        format_ident!("{}", self.get_module_name())
    }

    fn get_identifier_snake_case(&self) -> Ident {
        format_ident!("{}", self.get_module_name().to_case(Case::Snake))
    }

    fn get_path_segments(&self) -> Vec<Ident> {
        self.get_module()
            .path_segments
            .iter()
            .map(|segment| format_ident!("{}", segment))
            .collect()
    }

    fn get_field(&self) -> TokenStream {
        let module_name_identifier_snake_case = self.get_identifier_snake_case();
        let module_name_identifier = self.get_identifier();
        let path_segments = self.get_path_segments();
        let cycler_module_name_identifier = format_ident!("{}", self.get_module().cycler_module);

        quote! {
            #module_name_identifier_snake_case:
                #cycler_module_name_identifier::#(#path_segments::)*#module_name_identifier
        }
    }

    fn get_initializer_field_initializers(&self) -> anyhow::Result<Vec<TokenStream>> {
        self.get_module()
            .contexts
            .new_context
            .iter()
            .map(|field| match field {
                Field::AdditionalOutput { name, .. } => {
                    bail!("Unexpected additional output field `{name}` in new context")
                }
                Field::HardwareInterface { name } => Ok(quote! {
                    #name: framework::HardwareInterface::from(
                        &hardware_interface,
                    )
                }),
                Field::HistoricInput { name, .. } => {
                    bail!("Unexpected historic input field `{name}` in new context")
                }
                Field::MainOutput { name, .. } => {
                    bail!("Unexpected main output field `{name}` in new context")
                }
                Field::OptionalInput { name, .. } => {
                    bail!("Unexpected optional input field `{name}` in new context")
                }
                Field::Parameter { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Ok(quote! {
                        #name: framework::Parameter::from(
                            &configuration #(.#segments)*,
                        )
                    })
                }
                Field::PerceptionInput { name, .. } => {
                    bail!("Unexpected perception input field `{name}` in new context")
                }
                Field::PersistentState { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Ok(quote! {
                        #name: framework::PersistentState::from(
                            &mut persistent_state #(.#segments)*,
                        )
                    })
                }
                Field::RequiredInput { name, .. } => {
                    bail!("Unexpected required input field `{name}` in new context")
                }
            })
            .collect()
    }

    fn get_initializer(&self) -> anyhow::Result<TokenStream> {
        let module_name_identifier_snake_case = self.get_identifier_snake_case();
        let module_name_identifier = self.get_identifier();
        let path_segments = self.get_path_segments();
        let cycler_module_name_identifier = format_ident!("{}", self.get_module().cycler_module);
        let field_initializers = self
            .get_initializer_field_initializers()
            .context("Failed to generate field initializers")?;
        let error_message = format!("Failed to create module `{}`", self.get_module_name());

        Ok(quote! {
            let #module_name_identifier_snake_case = #cycler_module_name_identifier::#(#path_segments::)*#module_name_identifier::new(
                #cycler_module_name_identifier::#(#path_segments::)::*NewContext {
                    #(#field_initializers,)*
                },
            )
            .context(#error_message)?;
        })
    }

    fn get_required_inputs_are_some(&self) -> Option<TokenStream> {
        let required_inputs_are_some: Vec<_> = self
            .get_module()
            .contexts
            .cycle_context
            .iter()
            .filter_map(|field| match field {
                Field::RequiredInput { .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Some(quote! {
                        own_database.main_outputs #(.#segments)* .is_some()
                    })
                }
                _ => None,
            })
            .collect();
        match required_inputs_are_some.is_empty() {
            true => None,
            false => Some(quote! {
                if #(#required_inputs_are_some&&)*
            }),
        }
    }

    fn get_execution_field_initializers(&self) -> anyhow::Result<Vec<TokenStream>> {
        self.get_module()
            .contexts
            .cycle_context
            .iter()
            .map(|field| match field {
                Field::AdditionalOutput { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    // TODO: is_subscribed
                    Ok(quote! {
                        #name: framework::AdditionalOutput::new(
                            false,
                            &mut own_database.additional_outputs #(.#segments)*,
                        )
                    })
                }
                Field::HardwareInterface { name } => Ok(quote! {
                    #name: framework::HardwareInterface::from(
                        &self.hardware_interface,
                    )
                }),
                Field::HistoricInput { name, .. } => {
                    // TODO
                    // bail!("Unexpected historic input field `{name}` in cycle context")
                    Ok(quote! {
                        #name: todo!()
                    })
                }
                Field::MainOutput { name, .. } => {
                    bail!("Unexpected main output field `{name}` in cycle context")
                }
                Field::OptionalInput { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Ok(quote! {
                        #name: framework::OptionalInput::from(
                            &own_database.main_outputs #(.#segments)*,
                        )
                    })
                }
                Field::Parameter { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Ok(quote! {
                        #name: framework::Parameter::from(
                            &configuration #(.#segments)*,
                        )
                    })
                }
                Field::PerceptionInput { name, .. } => {
                    // TODO
                    // bail!("Unexpected perception input field `{name}` in cycle context")
                    Ok(quote! {
                        #name: todo!()
                    })
                }
                Field::PersistentState { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Ok(quote! {
                        #name: framework::PersistentState::from(
                            &mut persistent_state #(.#segments)*,
                        )
                    })
                }
                Field::RequiredInput { name, .. } => {
                    let segments = field
                        .get_path_segments()
                        .unwrap()
                        .into_iter()
                        .map(|segment| format_ident!("{}", segment));
                    Ok(quote! {
                        #name: framework::RequiredInput::from(
                            own_database.main_outputs #(.#segments)*.as_ref().unwrap(),
                        )
                    })
                }
            })
            .collect()
    }

    fn get_main_output_setters_from_cycle_result(&self) -> Vec<TokenStream> {
        self.get_module()
            .contexts
            .main_outputs
            .iter()
            .filter_map(|field| match field {
                Field::MainOutput { name, .. } => Some(quote! {
                    own_database.main_outputs.#name = main_outputs.#name.value;
                }),
                _ => None,
            })
            .collect()
    }

    fn get_main_output_setters_from_none(&self) -> Vec<TokenStream> {
        self.get_module()
            .contexts
            .main_outputs
            .iter()
            .filter_map(|field| match field {
                Field::MainOutput { name, .. } => Some(quote! {
                    own_database.main_outputs.#name = None;
                }),
                _ => None,
            })
            .collect()
    }

    fn get_execution(&self) -> anyhow::Result<TokenStream> {
        let module_name_identifier_snake_case = self.get_identifier_snake_case();
        let path_segments = self.get_path_segments();
        let cycler_module_name_identifier = format_ident!("{}", self.get_module().cycler_module);
        let required_inputs_are_some = self.get_required_inputs_are_some();
        let field_initializers = self
            .get_execution_field_initializers()
            .context("Failed to generate field initializers")?;
        let main_output_setters_from_cycle_result =
            self.get_main_output_setters_from_cycle_result();
        let main_output_setters_from_none = self.get_main_output_setters_from_none();
        let error_message = format!(
            "Failed to execute cycle of module `{}`",
            self.get_module_name()
        );
        let module_execution = quote! {
            let main_outputs = self.#module_name_identifier_snake_case.cycle(
                #cycler_module_name_identifier::#(#path_segments::)::*CycleContext {
                    #(#field_initializers,)*
                },
            )
            .context(#error_message)?;
            #(#main_output_setters_from_cycle_result)*
        };

        match required_inputs_are_some {
            Some(required_inputs_are_some) => Ok(quote! {
                #required_inputs_are_some {
                    #module_execution
                } else {
                    #(#main_output_setters_from_none)*
                }
            }),
            None => Ok(quote! {
                {
                    #module_execution
                }
            }),
        }
    }
}
