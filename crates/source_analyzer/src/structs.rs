use std::{collections::BTreeMap, iter::once, path::Path};

use anyhow::{bail, Context};
use quote::ToTokens;
use syn::Type;

use crate::{expand_variables_from_path, CyclerInstances, Field, Modules, PathSegment};

#[derive(Debug, Default)]
pub struct Structs {
    pub configuration: StructHierarchy,
    pub cycler_structs: BTreeMap<String, CyclerStructs>,
}

impl Structs {
    pub fn try_from_crates_directory<P>(crates_directory: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let mut structs = Self::default();

        let cycler_instances = CyclerInstances::try_from_crates_directory(&crates_directory)
            .context("Failed to get cycler instances")?;
        let modules = Modules::try_from_crates_directory(&crates_directory)
            .context("Failed to get modules")?;

        for (cycler_module, module_names) in modules.cycler_modules_to_modules.iter() {
            let cycler_structs = structs
                .cycler_structs
                .entry(cycler_module.clone())
                .or_default();
            let cycler_instances = &cycler_instances.modules_to_instances[cycler_module];

            for module_name in module_names.iter() {
                let contexts = &modules.modules[module_name].contexts;

                for field in contexts.main_outputs.iter() {
                    match field {
                        Field::MainOutput { data_type, name } => {
                            match &mut cycler_structs.main_outputs {
                                StructHierarchy::Struct { fields } => {
                                    fields.insert(
                                        name.to_string(),
                                        StructHierarchy::Field {
                                            data_type: data_type.clone(),
                                        },
                                    );
                                }
                                _ => bail!("Unexpected non-struct hierarchy in main outputs"),
                            }
                        }
                        _ => {
                            bail!("Unexpected field {field:?} in MainOutputs");
                        }
                    }
                }
                for field in contexts
                    .new_context
                    .iter()
                    .chain(contexts.cycle_context.iter())
                {
                    match field {
                        Field::AdditionalOutput {
                            data_type, path, ..
                        } => {
                            let expanded_paths = expand_variables_from_path(
                                path,
                                &BTreeMap::from_iter([(
                                    "cycler_instance".to_string(),
                                    cycler_instances.clone(),
                                )]),
                            )
                            .context("Failed to expand path variables")?;

                            for path in expanded_paths {
                                let insertion_rules = path_into_insertion_rules(&path, data_type);
                                cycler_structs
                                    .additional_outputs
                                    .insert(insertion_rules)
                                    .context(
                                        "Failed to insert expanded path into additional outputs",
                                    )?;
                            }
                        }
                        Field::Parameter {
                            data_type, path, ..
                        } => {
                            let expanded_paths = expand_variables_from_path(
                                path,
                                &BTreeMap::from_iter([(
                                    "cycler_instance".to_string(),
                                    cycler_instances.clone(),
                                )]),
                            )
                            .context("Failed to expand path variables")?;

                            for path in expanded_paths {
                                let insertion_rules = path_into_insertion_rules(&path, data_type);
                                structs
                                    .configuration
                                    .insert(insertion_rules)
                                    .context("Failed to insert expanded path into configuration")?;
                            }
                        }
                        Field::PersistentState {
                            data_type, path, ..
                        } => {
                            let expanded_paths = expand_variables_from_path(
                                path,
                                &BTreeMap::from_iter([(
                                    "cycler_instance".to_string(),
                                    cycler_instances.clone(),
                                )]),
                            )
                            .context("Failed to expand path variables")?;

                            for path in expanded_paths {
                                let insertion_rules = path_into_insertion_rules(&path, data_type);
                                cycler_structs
                                    .persistent_state
                                    .insert(insertion_rules)
                                    .context(
                                        "Failed to insert expanded path into persistent state",
                                    )?;
                            }
                        }
                        Field::HardwareInterface { .. }
                        | Field::HistoricInput { .. }
                        | Field::OptionalInput { .. }
                        | Field::PerceptionInput { .. }
                        | Field::RequiredInput { .. } => {}
                        _ => {
                            bail!("Unexpected field {field:?} in NewContext or CycleContext");
                        }
                    }
                }
            }
        }

        Ok(structs)
    }
}

#[derive(Debug, Default)]
pub struct CyclerStructs {
    pub main_outputs: StructHierarchy,
    pub additional_outputs: StructHierarchy,
    pub persistent_state: StructHierarchy,
}

#[derive(Debug)]
pub enum StructHierarchy {
    Struct {
        fields: BTreeMap<String, StructHierarchy>,
    },
    Optional {
        child: Box<StructHierarchy>,
    },
    Field {
        data_type: Type,
    },
}

impl Default for StructHierarchy {
    fn default() -> Self {
        Self::Struct {
            fields: Default::default(),
        }
    }
}

impl StructHierarchy {
    fn insert(&mut self, mut insertion_rules: Vec<InsertionRule>) -> anyhow::Result<()> {
        let first_rule = match insertion_rules.first() {
            Some(first_rule) => first_rule,
            None => return Ok(()),
        };

        match self {
            StructHierarchy::Struct { fields } => match first_rule {
                InsertionRule::InsertField { name } => fields
                    .entry(name.clone())
                    .or_default()
                    .insert(insertion_rules.split_off(1)),
                InsertionRule::BeginOptional => {
                    if !fields.is_empty() {
                        bail!("Failed to begin optional in-place of non-empty struct");
                    }
                    let mut child = StructHierarchy::default();
                    child.insert(insertion_rules.split_off(1))?;
                    *self = StructHierarchy::Optional {
                        child: Box::new(child),
                    };
                    Ok(())
                }
                InsertionRule::BeginStruct => self.insert(insertion_rules.split_off(1)),
                InsertionRule::AppendDataType { data_type } => {
                    if !fields.is_empty() {
                        bail!("Failed to append data type in-place of non-empty struct");
                    }
                    *self = StructHierarchy::Field {
                        data_type: data_type.clone(),
                    };
                    Ok(())
                }
            },
            StructHierarchy::Optional { .. } => match first_rule {
                InsertionRule::InsertField { name } => {
                    bail!("Failed to insert field with name `{name}` to optional")
                }
                InsertionRule::BeginOptional => self.insert(insertion_rules.split_off(1)),
                InsertionRule::BeginStruct => bail!("Failed to begin struct in-place of optional"),
                InsertionRule::AppendDataType { .. } => {
                    bail!("Failed to append data type in-place of optional")
                }
            },
            StructHierarchy::Field { data_type } => match first_rule {
                InsertionRule::InsertField { .. } => Ok(()),
                InsertionRule::BeginOptional => Ok(()),
                InsertionRule::BeginStruct => Ok(()),
                InsertionRule::AppendDataType {
                    data_type: data_type_to_be_appended,
                } => {
                    if data_type != data_type_to_be_appended {
                        bail!(
                            "Unmatching data types: previous data type {} does not match data type {} to be appended",
                            data_type.to_token_stream(),
                            data_type_to_be_appended.to_token_stream(),
                        );
                    }
                    Ok(())
                }
            },
        }
    }
}

#[derive(Clone, Debug)]
enum InsertionRule {
    InsertField { name: String },
    BeginOptional,
    BeginStruct,
    AppendDataType { data_type: Type },
}

fn path_into_insertion_rules(path: &[PathSegment], data_type: &Type) -> Vec<InsertionRule> {
    path.iter()
        .map(|segment| {
            assert_eq!(segment.is_variable, false);
            match segment.is_optional {
                true => vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: segment.name.clone(),
                    },
                    InsertionRule::BeginOptional,
                ],
                false => vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: segment.name.clone(),
                    },
                ],
            }
        })
        .flatten()
        .chain(once(InsertionRule::AppendDataType {
            data_type: data_type.clone(),
        }))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_expand_to_correct_insertion_rules() {
        let data_type = Type::Verbatim(Default::default());
        let cases = [
            (
                "a/b/c",
                vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "a".to_string(),
                    },
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "b".to_string(),
                    },
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "c".to_string(),
                    },
                    InsertionRule::AppendDataType {
                        data_type: data_type.clone(),
                    },
                ],
            ),
            (
                "a?/b/c",
                vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "a".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "b".to_string(),
                    },
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "c".to_string(),
                    },
                    InsertionRule::AppendDataType {
                        data_type: data_type.clone(),
                    },
                ],
            ),
            (
                "a?/b?/c",
                vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "a".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "b".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "c".to_string(),
                    },
                    InsertionRule::AppendDataType {
                        data_type: data_type.clone(),
                    },
                ],
            ),
            (
                "a?/b?/c?",
                vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "a".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "b".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "c".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::AppendDataType {
                        data_type: data_type.clone(),
                    },
                ],
            ),
            (
                "a/b?/c?",
                vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "a".to_string(),
                    },
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "b".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "c".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::AppendDataType {
                        data_type: data_type.clone(),
                    },
                ],
            ),
            (
                "a/b/c?",
                vec![
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "a".to_string(),
                    },
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "b".to_string(),
                    },
                    InsertionRule::BeginStruct,
                    InsertionRule::InsertField {
                        name: "c".to_string(),
                    },
                    InsertionRule::BeginOptional,
                    InsertionRule::AppendDataType {
                        data_type: data_type.clone(),
                    },
                ],
            ),
        ];

        for case in cases {
            let path = case.0;
            let path_segments: Vec<_> = path.split('/').map(PathSegment::from).collect();
            let insertion_rules = path_into_insertion_rules(&path_segments, &data_type);
            let expected_insertion_rules = case.1;

            assert_eq!(insertion_rules.len(), expected_insertion_rules.len(), "path: {path:?}, insertion_rules: {insertion_rules:?}, expected_insertion_rules: {expected_insertion_rules:?}");
            for (insertion_rule, expected_insertion_rule) in insertion_rules
                .into_iter()
                .zip(expected_insertion_rules.into_iter())
            {
                match (&insertion_rule, &expected_insertion_rule) {
                    (InsertionRule::InsertField { name }, InsertionRule::InsertField { name: expected_name }) if name == expected_name => {},
                    (InsertionRule::BeginOptional, InsertionRule::BeginOptional) => {},
                    (InsertionRule::BeginStruct, InsertionRule::BeginStruct) => {},
                    (InsertionRule::AppendDataType { data_type }, InsertionRule::AppendDataType { data_type: expected_data_type }) if data_type == expected_data_type => {},
                    _ => panic!("Insertion rule does not match expected: insertion_rule = {insertion_rule:?}, expected_insertion_rule = {expected_insertion_rule:?}"),
                }
            }
        }
    }

    #[test]
    fn insertion_rules_without_optionals_result_in_correct_struct_hierarchy() {
        let data_type = Type::Verbatim(Default::default());
        let insertion_rules = vec![
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "a".to_string(),
            },
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "b".to_string(),
            },
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "c".to_string(),
            },
            InsertionRule::AppendDataType {
                data_type: data_type.clone(),
            },
        ];
        let mut hierarchy = StructHierarchy::default();
        hierarchy.insert(insertion_rules).unwrap();

        assert!(
            match &hierarchy {
                StructHierarchy::Struct { fields }
                    if fields.len() == 1
                        && match fields.get(&"a".to_string()) {
                            Some(a) => match a {
                                StructHierarchy::Struct { fields }
                                    if fields.len() == 1
                                        && match fields.get(&"b".to_string()) {
                                            Some(b) => match b {
                                                StructHierarchy::Struct { fields }
                                                    if fields.len() == 1
                                                        && match fields.get(&"c".to_string()) {
                                                            Some(c) => match c {
                                                                StructHierarchy::Field {
                                                                    data_type: matched_data_type,
                                                                } if &data_type
                                                                    == matched_data_type =>
                                                                {
                                                                    true
                                                                }
                                                                _ => false,
                                                            },
                                                            None => false,
                                                        } =>
                                                    true,
                                                _ => false,
                                            },
                                            None => false,
                                        } =>
                                    true,
                                _ => false,
                            },
                            None => false,
                        } =>
                    true,
                _ => false,
            },
            "{hierarchy:?} does not match"
        );
    }

    #[test]
    fn insertion_rules_with_one_optional_result_in_correct_struct_hierarchy() {
        let data_type = Type::Verbatim(Default::default());
        let insertion_rules = vec![
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "a".to_string(),
            },
            InsertionRule::BeginOptional,
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "b".to_string(),
            },
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "c".to_string(),
            },
            InsertionRule::AppendDataType {
                data_type: data_type.clone(),
            },
        ];
        let mut hierarchy = StructHierarchy::default();
        hierarchy.insert(insertion_rules).unwrap();

        assert!(
            match &hierarchy {
                StructHierarchy::Struct { fields }
                    if fields.len() == 1
                        && match fields.get(&"a".to_string()) {
                            Some(a) => match a {
                                StructHierarchy::Optional { child } => match &**child {
                                    StructHierarchy::Struct { fields }
                                        if fields.len() == 1
                                            && match fields.get(&"b".to_string()) {
                                                Some(b) => match b {
                                                    StructHierarchy::Struct { fields }
                                                        if fields.len() == 1
                                                            && match fields.get(&"c".to_string()) {
                                                                Some(c) => match c {
                                                                    StructHierarchy::Field {
                                                                        data_type: matched_data_type,
                                                                    } if &data_type
                                                                        == matched_data_type =>
                                                                    {
                                                                        true
                                                                    }
                                                                    _ => false,
                                                                },
                                                                None => false,
                                                            } =>
                                                        true,
                                                    _ => false,
                                                },
                                                None => false,
                                            } =>
                                        true,
                                    _ => false,
                                },
                                _ => false,
                            },
                            None => false,
                        } =>
                    true,
                _ => false,
            },
            "{hierarchy:?} does not match"
        );
    }

    #[test]
    fn insertion_rules_with_two_optionals_result_in_correct_struct_hierarchy() {
        let data_type = Type::Verbatim(Default::default());
        let insertion_rules = vec![
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "a".to_string(),
            },
            InsertionRule::BeginOptional,
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "b".to_string(),
            },
            InsertionRule::BeginOptional,
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "c".to_string(),
            },
            InsertionRule::AppendDataType {
                data_type: data_type.clone(),
            },
        ];
        let mut hierarchy = StructHierarchy::default();
        hierarchy.insert(insertion_rules).unwrap();

        assert!(
            match &hierarchy {
                StructHierarchy::Struct { fields }
                    if fields.len() == 1
                        && match fields.get(&"a".to_string()) {
                            Some(a) => match a {
                                StructHierarchy::Optional { child } => match &**child {
                                    StructHierarchy::Struct { fields }
                                        if fields.len() == 1
                                            && match fields.get(&"b".to_string()) {
                                                Some(b) => match b {
                                                    StructHierarchy::Optional { child } => match &**child {
                                                        StructHierarchy::Struct { fields }
                                                            if fields.len() == 1
                                                                && match fields.get(&"c".to_string()) {
                                                                    Some(c) => match c {
                                                                        StructHierarchy::Field {
                                                                            data_type: matched_data_type,
                                                                        } if &data_type
                                                                            == matched_data_type =>
                                                                        {
                                                                            true
                                                                        }
                                                                        _ => false,
                                                                    },
                                                                    None => false,
                                                                } =>
                                                            true,
                                                        _ => false,
                                                    },
                                                    _ => false,
                                                },
                                                None => false,
                                            } =>
                                        true,
                                    _ => false,
                                },
                                _ => false,
                            },
                            None => false,
                        } =>
                    true,
                _ => false,
            },
            "{hierarchy:?} does not match"
        );
    }

    #[test]
    fn insertion_rules_with_three_optionals_result_in_correct_struct_hierarchy() {
        let data_type = Type::Verbatim(Default::default());
        let insertion_rules = vec![
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "a".to_string(),
            },
            InsertionRule::BeginOptional,
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "b".to_string(),
            },
            InsertionRule::BeginOptional,
            InsertionRule::BeginStruct,
            InsertionRule::InsertField {
                name: "c".to_string(),
            },
            InsertionRule::BeginOptional,
            InsertionRule::AppendDataType {
                data_type: data_type.clone(),
            },
        ];
        let mut hierarchy = StructHierarchy::default();
        hierarchy.insert(insertion_rules).unwrap();

        assert!(
            match &hierarchy {
                StructHierarchy::Struct { fields }
                    if fields.len() == 1
                        && match fields.get(&"a".to_string()) {
                            Some(a) => match a {
                                StructHierarchy::Optional { child } => match &**child {
                                    StructHierarchy::Struct { fields }
                                        if fields.len() == 1
                                            && match fields.get(&"b".to_string()) {
                                                Some(b) => match b {
                                                    StructHierarchy::Optional { child } => match &**child {
                                                        StructHierarchy::Struct { fields }
                                                            if fields.len() == 1
                                                                && match fields.get(&"c".to_string()) {
                                                                    Some(c) => match c {
                                                                        StructHierarchy::Optional { child } => match &**child {
                                                                            StructHierarchy::Field {
                                                                                data_type: matched_data_type,
                                                                            } if &data_type
                                                                                == matched_data_type =>
                                                                            {
                                                                                true
                                                                            }
                                                                            _ => false,
                                                                        },
                                                                        _ => false,
                                                                    },
                                                                    None => false,
                                                                } =>
                                                            true,
                                                        _ => false,
                                                    },
                                                    _ => false,
                                                },
                                                None => false,
                                            } =>
                                        true,
                                    _ => false,
                                },
                                _ => false,
                            },
                            None => false,
                        } =>
                    true,
                _ => false,
            },
            "{hierarchy:?} does not match"
        );
    }
}
