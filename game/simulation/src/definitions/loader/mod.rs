pub use load::load;
pub use step1_deserialization::{DefinitionSource, DefinitionUid};
pub use step3_construction::Definition;

pub type ValueImpl = ron::Value;

#[cfg(test)]
pub use load::load_from_str;

mod helpers;
mod load;
mod step1_deserialization;
mod step2_preprocessing;
mod step3_construction;
mod template_lookup;

// TODO consider using `nested` vecs as an optimization
#[cfg(test)]
mod tests {
    use specs::EntityBuilder;

    use crate::definitions::loader::load::preprocess_from_str;
    use crate::definitions::loader::step1_deserialization::{
        DefinitionUid, DeserializedDefinition,
    };
    use crate::definitions::loader::step3_construction::{instantiate, Definition};
    use crate::definitions::loader::template_lookup::TemplateLookup;
    use crate::definitions::DefinitionErrorKind;
    use crate::ecs::{ComponentBuildError, ComponentTemplate, ComponentTemplateEntry, Map};

    use super::*;
    use common::LevelFilter;
    use env_logger;

    #[test]
    fn example() {
        let input = r#"
[
	(
		uid: "myuid",
		components: [
			 {"item": (
			 	name: "Apple",
			 	base_slots: 1,
			 	mounted_slots: 1,
			 	mass: 0.1,
			 	stacking: "item_default",
			 )},
			{"throwable": ()},
			{"edible": (total_nutrition: 80)},
		],
	)
]
        "#;

        let definitions =
            instantiate(preprocess_from_str(input).unwrap(), &TemplateLookup::init()).unwrap();
        let (uid, d) = &definitions[0];
        assert_eq!(uid, "myuid");
        assert_eq!(d.components().count(), 3);
    }

    #[test]
    fn duplicates() {
        let input = r#"
[
	(
		uid: "uhoh",
		components: [
			{"comp1": ()},
			{"comp2": ()},
			{"comp1": ()},
		],
	)
]
        "#;

        let errs = preprocess_from_str(input).expect_err("should fail");
        assert_eq!(errs.0.len(), 1);
    }

    #[derive(Debug)]
    struct TestComponentTemplate {
        int: i32,
        string: String,
    }

    impl ComponentTemplate<ValueImpl> for TestComponentTemplate {
        fn construct(
            values: &mut Map<ValueImpl>,
        ) -> Result<Box<dyn ComponentTemplate<ValueImpl>>, ComponentBuildError>
        where
            Self: Sized,
        {
            Ok(Box::new(Self {
                int: values.get_int("int")?,
                string: values.get_string("string")?,
            }))
        }

        fn instantiate<'b>(&self, builder: EntityBuilder<'b>) -> EntityBuilder<'b> {
            unimplemented!()
        }
    }

    crate::register_component_template!("TESTcomp", TestComponentTemplate);

    fn get_test_template<'a>(
        defs: &[(DefinitionUid, Definition)],
        def_idx: usize,
    ) -> &'a TestComponentTemplate {
        let ptr = defs[def_idx].1.components().next().unwrap();
        let template = &*ptr;
        let ptr = template as *const dyn ComponentTemplate<ValueImpl> as *const u8;
        #[allow(clippy::transmute_ptr_to_ref)]
        unsafe {
            std::mem::transmute(ptr)
        }
    }

    #[test]
    fn circular_reference() {
        let input = r#"
[
    (
        uid: "hello",
        parent: "goodbye",
        components: [],
    ),
    (
        uid: "goodbye",
        parent: "hello",
        components: [],
    ),
]
        "#;

        let errs = preprocess_from_str(input).expect_err("should fail");
        let err = &errs.0[0].1;
        assert!(matches!(
            *dbg!(err),
            DefinitionErrorKind::CyclicParentRelation(_, _)
        ));

        let input = r#"
[
    (
        uid: "myself",
        parent: "myself",
        components: [],
    ),
]
        "#;

        let errs = preprocess_from_str(input).expect_err("should fail");
        let err = &errs.0[0].1;
        assert!(matches!(
            *err,
            DefinitionErrorKind::CyclicParentRelation(_, _)
        ));
    }

    #[test]
    fn inheritance() {
        let _ = env_logger::builder()
            .filter_level(LevelFilter::Trace)
            .is_test(true)
            .try_init();

        let input = r#"
[
    (
        uid: "test_a",
        components: [
             {"nice": (
                name: "thing a",
                int: 100,
             )},
            {"cool": ()},
            {"sweet": (int: 500)},

        ],
    ),
    (
        uid: "test_b",
        parent: "test_a",

        components: [
             {"nice": (
                name: "different thing",
                // inherit int: 100
             )},
            {"cool": None}, // remove cool component
            // inherit sweet as-is
            {"epic": (unbelievable: 202)},
        ],
    ),
    (
        uid: "test_c",
        parent: "test_b", // grandchild
        components: [
            {"cool": ()}, // add back cool
        ],
    ),
    (
        uid: "test_d",
        parent: "test_a", // sibling of test_b
        components: [], // inherits all
    ),
]
        "#;

        let definitions = preprocess_from_str(input).expect("should success");
        let a = dbg!(&definitions[0]);
        let b = dbg!(&definitions[1]);
        let c = dbg!(&definitions[2]);
        let d = dbg!(&definitions[3]);

        assert_eq!(a.uid(), "test_a");
        assert_eq!(b.uid(), "test_b");
        assert_eq!(c.uid(), "test_c");
        assert_eq!(d.uid(), "test_d");

        assert_eq!(a.processed_components().len(), 3);
        assert_eq!(
            b.processed_components().len(),
            a.processed_components().len()
        );

        let get_comps = |def: &DeserializedDefinition| {
            let comps = def.processed_components();

            let get_comp = |name| {
                comps
                    .iter()
                    .find(|(comp_name, _)| name == comp_name)
                    .map(|(_, fields)| fields.to_owned())
            };

            (
                get_comp("nice"),
                get_comp("cool"),
                get_comp("sweet"),
                get_comp("epic"),
            )
        };

        let (nice_a, cool_a, sweet_a, epic_a) = get_comps(a);
        let (nice_b, cool_b, sweet_b, epic_b) = get_comps(b);

        use ron::Value::*;

        // b overrides field from a
        let nice_a = nice_a.unwrap();
        let nice_b = nice_b.unwrap();
        assert_eq!(*nice_a.field("name").unwrap(), String("thing a".to_owned()));
        assert_eq!(
            *nice_b.field("name").unwrap(),
            String("different thing".to_owned())
        );

        // b inherits field from a
        assert_eq!(
            *nice_a.field("int").unwrap(),
            Number(ron::Number::Integer(100))
        );
        assert_eq!(
            *nice_b.field("int").unwrap(),
            Number(ron::Number::Integer(100))
        );

        // a has cool comp, but b negates/removes it
        assert!(cool_a.is_some());
        assert!(cool_b.is_none());

        // b inherits all of sweet from a
        let sweet_a = sweet_a.unwrap();
        let sweet_b = sweet_b.unwrap();
        assert_eq!(
            *sweet_a.field("int").unwrap(),
            Number(ron::Number::Integer(500))
        );
        assert_eq!(
            *sweet_b.field("int").unwrap(),
            Number(ron::Number::Integer(500))
        );

        // b adds an epic of its own
        assert!(epic_a.is_none());
        let epic_b = epic_b.unwrap();
        assert_eq!(
            *epic_b.field("unbelievable").unwrap(),
            Number(ron::Number::Integer(202))
        );

        let (nice_c, cool_c, _, _) = get_comps(c);

        // c adds back its own cool after its parent removed it from its grandparent
        assert!(cool_c.is_some());

        // c inherits nice from parent and grandparent
        let nice_c = nice_c.unwrap();
        assert_eq!(
            *nice_c.field("name").unwrap(),
            String("different thing".to_owned())
        ); // from parent
        assert_eq!(
            *nice_c.field("int").unwrap(),
            Number(ron::Number::Integer(100)) // from grandparent
        );

        // d inherits all components with no overrides
        let (nice_d, cool_d, sweet_d, epic_d) = get_comps(d);
        assert!(nice_d.is_some());
        assert!(cool_d.is_some());
        assert!(sweet_d.is_some());
        assert!(epic_d.is_none());
    }

    #[test]
    fn bad_uid() {
        let input = r#"
[
    (
        uid: "terrible uid!",
        components: [],
    ),
    (
        uid: "bad-uid",
        components: [],
    ),
    (
        uid: "good_uid",
        components: [],
    ),
]
        "#;

        let errs = preprocess_from_str(input).expect_err("should fail");
        assert_eq!(errs.0.len(), 2);
    }

    #[test]
    fn abstract_base() {
        let input = r#"
[
    (
        uid: "base",
        abstract: true,
        components: [
            {"nice": ()},
            {"cool": ()},
            {"sweet": (int: 500)},
        ],
    ),
    (
        uid: "real",
        parent: "base",

        components: [],
    ),
]
        "#;

        let definitions = preprocess_from_str(input).expect("should success");
        assert_eq!(definitions.len(), 1); // only "real"
        let real = dbg!(&definitions[0]);
        assert_eq!(real.uid(), "real");
        assert_eq!(real.processed_components().len(), 3);
    }
}
