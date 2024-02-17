use std::{
    collections::{btree_map::Entry, HashMap},
    io::{Read, Seek},
};

use getset::Getters;

use crate::{
    low_level::{self, BasicGeometry},
    Gdtf, GdtfError, GdtfParseError, Geometry, GeometryType, HandleProblem, Name, PlaceGdtfError,
    Problem, Problems, ProblemsMut,
};

use super::low_level::ParsedGdtf;

#[derive(Debug, Getters)]
#[getset(get = "pub")]
pub struct ValidatedGdtf {
    gdtf: Gdtf,
    problems: Problems,
}

impl ValidatedGdtf {
    pub fn from_reader<T: Read + Seek>(reader: T) -> Result<Self, GdtfParseError> {
        let low_level_parsed = ParsedGdtf::from_reader(reader)?;
        Ok(low_level_parsed.into())
    }
}

impl From<ParsedGdtf> for ValidatedGdtf {
    fn from(mut parsed: ParsedGdtf) -> Self {
        let mut gdtf = Gdtf::default();

        validate_geometries(&mut parsed, &mut gdtf);

        gdtf.data_version = parsed.gdtf.data_version;
        gdtf.name = parsed.gdtf.fixture_type.name;
        gdtf.short_name = parsed.gdtf.fixture_type.short_name;
        gdtf.long_name = parsed.gdtf.fixture_type.long_name;
        gdtf.manufacturer = parsed.gdtf.fixture_type.manufacturer;
        gdtf.description = parsed.gdtf.fixture_type.description;
        gdtf.fixture_type_id = parsed.gdtf.fixture_type.id;
        gdtf.ref_ft = parsed.gdtf.fixture_type.ref_ft;
        gdtf.can_have_children = bool::from(parsed.gdtf.fixture_type.can_have_children);

        ValidatedGdtf {
            gdtf,
            problems: parsed.problems,
        }
    }
}

fn validate_geometries(parsed: &mut ParsedGdtf, gdtf: &mut Gdtf) {
    let input = &parsed.gdtf.fixture_type.geometries.children;
    let p = &mut parsed.problems;

    for g in input.iter() {
        if let Some((to_add, children)) = translate_geometry(g, p) {
            let parent_name = to_add.name.clone();
            gdtf.add_top_level_geometry(to_add)
                .at("top level geometries")
                .ok_or_handled_by("ignoring node", p);
            maybe_add_children_geometries(children, gdtf, &parent_name, p);
        }
    }

    todo!("return renaming map");
}

fn maybe_add_children_geometries(
    children: Option<Vec<low_level::GeometryType>>,
    gdtf: &mut Gdtf,
    new_name: &Name,
    p: &mut impl ProblemsMut,
) {
    if let Some(children) = children {
        for c in children {
            validate_child_geometry_recursively(c, gdtf, new_name, p);
        }
    }
}

fn validate_child_geometry_recursively(
    g: low_level::GeometryType,
    gdtf: &mut Gdtf,
    parent: &Name,
    p: &mut impl ProblemsMut,
) -> Option<()> {
    let (to_add, children) = translate_geometry(&g, p)?;
    let new_name = to_add.name.clone();
    match gdtf.add_child_geometry(parent, to_add) {
        Ok(_) => {
            maybe_add_children_geometries(children, gdtf, &new_name, p);
        }
        Err(GdtfError::DuplicateGeometryName(n)) => todo!("rename"),
        r => {
            r.at(format!("geometry '{new_name}'"))
                .ok_or_handled_by("ignore geometry and its possible children", p);
        }
    }
    Some(())
}

fn translate_geometry(
    g: &low_level::GeometryType,
    p: &mut impl ProblemsMut,
) -> Option<(Geometry, Option<Vec<low_level::GeometryType>>)> {
    match g {
        crate::low_level::GeometryType::Geometry { basic, children } => Some((
            Geometry {
                name: basic.name.clone(),
                t: GeometryType::Geometry {
                    children: Vec::new(),
                },
            },
            Some(children.clone()),
        )),
        crate::low_level::GeometryType::GeometryReference {
            basic,
            geometry,
            breaks,
        } => Some((translate_reference(basic, geometry, breaks, p)?, None)),
    }
}

fn translate_reference(
    basic: &BasicGeometry,
    geometry: &Name,
    breaks: &Vec<low_level::Break>,
    p: &mut impl ProblemsMut,
) -> Option<Geometry> {
    let mut offsets = HashMap::new();
    let mut break_iter = breaks.iter().rev(); // start at last break which sets default (i.e. overwrite)

    let overwrite = break_iter
        .next()
        .map(|b| (b.dmx_break, b.dmx_offset.clone()))
        .ok_or(Problem::MissingBreakOffset().at_custom(basic.name.clone()))
        .ok_or_handled_by("ignoring geometry", p)?;

    break_iter.for_each(|b| match offsets.entry(b.dmx_break) {
        std::collections::hash_map::Entry::Occupied(entry) => {
            todo!("add to problems as duplicate, don't use it")
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(b.dmx_offset.clone());
        }
    });

    offsets.entry(overwrite.0).or_insert(overwrite.1.clone());

    Some(Geometry {
        name: basic.name.clone(),
        t: GeometryType::GeometryReference {
            geometry: geometry.clone(),
            overwrite,
            offsets,
        },
    })
}
